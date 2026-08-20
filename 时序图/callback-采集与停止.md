# callback 采集与停止

```mermaid
sequenceDiagram
    autonumber
    actor App as 业务代码
    participant Public as mv3d-lp 公共 API
    participant Core as Device 与 callback registry
    participant Native as 厂商 LPSDK

    App->>Public: device.register_image_callback(F)
    Public->>Core: Device::register_image_callback(Arc)
    alt acquisition = Pulling
        Core-->>Public: InvalidState
        Public-->>App: Err
    else acquisition = Idle、CallbackStopped 或 CallbackRunning
        Core->>Core: 创建不复用的 cookie
        Core->>Native: MV3D_LP_RegisterImageDataCallBack(...)
        Native-->>Core: register status
        alt Register 失败
            Core->>Core: 撤销本次 cookie，原状态与原 cookie 不变
            Core-->>Public: Err
            Public-->>App: Err
        else Register 成功
            Core->>Core: 保存新 cookie；Idle 进入 CallbackStopped，其余状态不变
            Core-->>Public: Ok
            Public-->>App: Ok
        end
    end

    App->>Public: device.start()
    Public->>Core: Device::start()
    alt acquisition = CallbackStopped
        Core->>Native: MV3D_LP_StartMeasure(handle)
        Native-->>Core: start status
        alt Start 成功
            Core->>Core: acquisition = CallbackRunning
            Core-->>Public: Ok
            Public-->>App: Ok
        else Start 失败
            Note over Core: CallbackStopped 与 registration 保存至 Close
            Core-->>Public: Err
            Public-->>App: Err
        end
    else 其他状态
        Core-->>Public: InvalidState
        Public-->>App: Err
    end

    loop 每次原生图像 callback
        Native->>Core: image_trampoline(image_ptr, cookie)
        Core->>Core: 查找 sink，校验并复制为 Image
        alt cookie 已撤销
            Core->>Core: 忽略
        else payload 有效
            Core->>App: F(Image)
        else descriptor 无效或空指针
            Core->>Core: 跳过本次投递
        else 用户 Fn panic
            Core->>Core: 静默该 cookie
        end
        Core-->>Native: trampoline 返回
    end

    App->>Public: device.stop()
    Public->>Core: Device::stop()
    alt acquisition = CallbackRunning
        Core->>Native: MV3D_LP_StopMeasure(handle)
        Native-->>Core: status
        alt Stop 成功
            Core->>Core: acquisition = CallbackStopped，registration 继续持有
            Core-->>Public: Ok
            Public-->>App: Ok
        else Stop 失败
            Core->>Core: acquisition 仍为 CallbackRunning
            Core-->>Public: Err
            Public-->>App: Err
        end
    else 其他状态
        Core-->>Public: InvalidState
        Public-->>App: Err
    end

    opt 显式 close 或 Drop
        Core->>Core: 取走 handle
        opt acquisition = CallbackRunning
            Core->>Native: MV3D_LP_StopMeasure(handle) 一次
            Native-->>Core: stop status
        end
        Core->>Native: MV3D_LP_CloseDevice(handle) 一次
        Native-->>Core: close status
        alt Close 失败
            Core->>Core: finalize_blocked = true
            Note over Core,Native: native handle 状态交给进程退出处理
        end
        Core->>Core: Close 返回后撤销 image 与 exception cookie
    end
```

Register 成功即把该 handle 绑定到 callback 至 Close。该规则覆盖后续 Start 失败和 Stop 成功。`CallbackStopped` 仍可再次 `register_image_callback()` 以替换 cookie，但不能回到 pull。Stop 成功后可再次 `start()`。

trampoline 在原生 callback 返回前复制全部 payload，再调用用户 `Fn(Image)`。非法 descriptor 或空指针跳过本次投递。用户代码 panic 时截获并静默该 cookie。cookie 只作不复用的整数标识，Rust 不解引用 user data。Close 返回后撤销 cookie；此前已取得的 sink clone 可以完成本次投递。仅 Close 成功代表 native callback 已静默，Close 失败则由 `finalize_blocked` 阻止 Finalize。

## exception delivery 停止

```mermaid
sequenceDiagram
    actor App as 业务代码
    participant Core as callback registry
    participant Native as 厂商 LPSDK

    opt 撤销前 callback 已开始
        Native->>Core: exception_trampoline(info, old_cookie)
        Core->>Core: 查找并 clone sink
    end
    App->>Core: device.disable_exception_delivery()
    Core->>Core: 撤销 exception cookie
    Core-->>App: 立即返回
    opt 已开始的 callback 持有 sink clone
        Core->>App: 完成本次 Fn 调用
        Core-->>Native: return
    end
    opt 撤销后到达的 callback
        Native->>Core: exception_trampoline(info, old_cookie)
        Core->>Core: 查询失败并忽略
        Core-->>Native: return
    end
```

`disable_exception_delivery()` 与 `disable_image_delivery()` 只停止后续 Rust delivery。

## 待确认厂商契约

- callback 参数在 ABI 上可为空，当前头文件与 sample 未说明传 `NULL` 是否表示注销；wrapper 因此仅撤销 Rust cookie。
