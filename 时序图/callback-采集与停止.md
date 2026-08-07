# callback 采集与停止

```mermaid
sequenceDiagram
    autonumber
    actor App as 业务代码
    participant Public as mv3d-lp 公共 API
    participant Queue as 有界 Rust channel
    participant Core as callback registry 与 trampoline
    participant Native as 厂商 LPSDK

    App->>Public: device.start_receiving(options)
    Public->>Queue: sync_channel(queue_capacity)
    Public->>Core: Device::start_callback(sink)
    Core->>Core: 创建 CallbackRegistration 和唯一 cookie
    Core->>Native: MV3D_LP_RegisterImageDataCallBack(handle, trampoline, cookie)
    Native-->>Core: status
    Core->>Native: MV3D_LP_StartMeasure(handle)
    Native-->>Core: status
    Core->>Core: Device 保存 registration，state = CallbackMeasuring
    Core-->>Public: Ok
    Public-->>App: Frame receiver

    loop 每次原生图像回调
        Native->>Core: image_trampoline(image_ptr, cookie)
        Core->>Core: registry 按 cookie 准入并登记 in-flight
        alt cookie 已撤销或未知
            Core-->>Native: 忽略晚到 callback
        else registration 正在接受事件
            Core->>Core: 校验并复制 payload 到 Rust 存储
            alt payload 有效
                Core->>Queue: try_send(frame)
                alt 队列有空位
                    Queue-->>Core: queued
                    Core->>Core: delivered += 1
                else 队列已满
                    Core->>Core: dropped_full += 1
                else receiver 已关闭
                    Core->>Core: fail closed，停止接受后续事件
                end
            else payload 无效
                Core->>Core: invalid_payloads += 1
            end
            Core->>Core: 结束 in-flight
            Core-->>Native: trampoline 返回
        end
    end

    Note over App,Queue: Receiver 的消费节奏与原生 callback 解耦
    loop 按业务节奏消费
        App->>Queue: receiver.recv()
        Queue-->>App: Frame
    end

    App->>Public: device.stop()
    Public->>Core: Device::stop()
    Core->>Core: 停止准入并从 registry 移除 cookie
    Core->>Core: 等待全部 in-flight callback 退出
    Core->>Queue: 释放 registration 持有的 sender
    Core->>Native: MV3D_LP_StopMeasure(handle)
    Native-->>Core: status
    alt Stop 成功
        Core->>Core: state = Open
        Core-->>Public: Ok
        Public-->>App: Ok
    else Stop 结果异常
        Core->>Core: state = Faulted
        Core-->>Public: Err
        Public-->>App: Err
    end

    opt Stop 失败后显式 close 或 Drop
        App->>Public: device.close() 或 drop(device)
        Public->>Core: Device cleanup
        Core->>Native: MV3D_LP_StopMeasure(handle) 再尝试一次
        Native-->>Core: retry status
        Core->>Native: MV3D_LP_CloseDevice(handle)
        Native-->>Core: close status
        Note over Core,Native: retry Stop 成败都继续 Close；显式 close 汇总清理错误
    end
```

`Frame` 入队前已复制 SDK payload；SDK handle、registration、cookie 与清理责任均留在 `Device`。已审计的原生 callback 接口只包含 register；wrapper 通过撤销 cookie 准入并等待 in-flight callback 退出来隔离晚到调用。若同一 handle 再次注册被厂商 runtime 接受，下一次 `start_receiving()` 会使用全新的 cookie；wrapper 不额外承诺设备或固件一定支持重复注册。完整状态图见[生命周期与时序图总览](../生命周期与时序图.md)。

## exception delivery 停止

```mermaid
sequenceDiagram
    actor App as 业务代码
    participant Worker as Rust worker / receiver
    participant Core as callback registry
    participant Native as 厂商 LPSDK

    App->>Core: device.disable_exception_delivery()
    Core->>Core: 停止准入并移除唯一 cookie
    Core->>Core: 等待已准入的 in-flight callback 返回
    Core->>Worker: 释放 sender
    Core-->>App: return
    App->>Worker: worker.join() 或等待 channel 断开
    opt 原生晚到 exception callback
        Native->>Core: exception_trampoline(info, retired_cookie)
        Core->>Core: registry 查询失败，忽略
        Core-->>Native: return
    end
```

厂商接口只提供 exception callback register，因此 `disable_exception_delivery()` 只撤销 Rust delivery。它可重复调用；原生侧仍可能使用旧 cookie 调用 trampoline，registry 会安全忽略。显式 disable 后再 join exception worker，可让 sender 正常释放。
