# 标准生命周期与 pull 采集

```mermaid
sequenceDiagram
    autonumber
    actor App as 业务代码
    participant Public as mv3d-lp 公共 API
    participant Core as mv3d-lp-internal
    participant Native as 厂商 LPSDK

    App->>Public: Sdk::version()
    Public->>Core: Runtime::version_bytes()
    Core->>Native: MV3D_LP_GetVersion()
    Native-->>Core: 原始版本字节
    Core->>Core: 复制为拥有字节，不解析段数或兼容区间
    Core-->>Public: SdkText
    Public-->>App: SdkText

    App->>Public: Sdk::initialize()
    Public->>Core: Runtime::initialize()
    Core->>Core: 原子领取本进程唯一初始化机会
    alt 初始化机会已被领取
        Core-->>Public: InvalidState，不调用 native
        Public-->>App: Err；本进程不重试
    else 首次 initialize
        Core->>Native: MV3D_LP_Initialize()
        Native-->>Core: status
        alt Initialize 成功
            Core->>Core: 创建 Arc&lt;RuntimeCore&gt;
            Core-->>Public: Runtime
            Public-->>App: Sdk
        else Initialize 失败
            Core-->>Public: Err；初始化机会仍已消费
            Public-->>App: Err；本进程不重试
        end
    end

    Note over App,Native: 以下为初始化成功后的主路径
    App->>Public: sdk.devices()
    Public->>Core: Runtime::devices()
    Core->>Native: MV3D_LP_GetDeviceNumber()
    Native-->>Core: count hint
    alt count = 0
        Core-->>Public: 空列表
    else count > 0
        Core->>Native: MV3D_LP_GetDeviceList(count)
        Native-->>Core: 设备描述符
        Core->>Core: 校验数量并复制为 DeviceRecord
        Core-->>Public: DeviceRecord 列表
    end
    Public-->>App: DeviceInfo 列表

    App->>Public: sdk.open_by_ip(address)
    Public->>Core: Runtime::open_by_ip(address)
    Core->>Native: MV3D_LP_OpenDeviceByIP(...)
    Native-->>Core: status + 原生 handle
    alt status 成功且 handle 非空
        Core->>Core: Device 获取 RuntimeCore 的 Arc owner，acquisition = Idle
        Core-->>Public: internal Device
        Public-->>App: Device
    else status 失败或成功时 handle 为空
        Core-->>Public: Err；不交付 handle
        Public-->>App: Err
    end

    opt 采集前配置
        App->>Public: device.set_parameter(node_name, value)
        Public->>Core: 转换 ParameterValue
        Core->>Native: MV3D_LP_SetParam(...)
        Native-->>Core: status
        Core-->>Public: Result
        Public-->>App: Result
    end

    App->>Public: device.start()
    Public->>Core: Device::start()
    alt acquisition = Idle
        Core->>Native: MV3D_LP_StartMeasure(handle)
        Native-->>Core: status
        alt StartMeasure 成功
            Core->>Core: acquisition = Pulling
            Core-->>Public: Ok
            Public-->>App: Ok
        else StartMeasure 失败
            Core-->>Public: Err，acquisition 仍为 Idle
            Public-->>App: Err
        end
    else 其他状态
        Core-->>Public: InvalidState
        Public-->>App: Err
    end

    loop 按需获取图像
        alt 有限等待
            App->>Public: device.get_image(timeout)
            Public->>Core: Device::get_image(timeout_ms)
        else 无限等待
            App->>Public: device.get_image_blocking()
            Public->>Core: Device::get_image(u32::MAX)
        end
        Core->>Native: MV3D_LP_GetImage(handle, descriptor, timeout_ms)
        Native-->>Core: status + 图像描述符和临时 payload 指针
        Core->>Core: status 成功后校验指针、长度、布局与算术
        Core->>Core: 立即复制主数据、亮度数据和曝光时间戳
        Core-->>Public: FrameRecord（拥有 payload）
        Public-->>App: Frame（Image 的别名）
    end

    App->>Public: device.stop()
    Public->>Core: Device::stop()
    Core->>Native: MV3D_LP_StopMeasure(handle)
    Native-->>Core: status
    alt Stop 成功
        Core->>Core: acquisition = Idle
        Core-->>Public: Ok
        Public-->>App: Ok
    else Stop 失败
        Core-->>Public: Err，acquisition 仍为 Pulling
        Public-->>App: Err
    end

    App->>Public: device.close()
    Public->>Core: Device::close() 消费 owner 并取走 handle
    opt acquisition = Pulling 或 CallbackRunning
        Core->>Native: MV3D_LP_StopMeasure(handle) 一次
        Native-->>Core: stop status
    end
    Core->>Native: MV3D_LP_CloseDevice(handle) 一次
    Native-->>Core: close status
    alt Close 成功
        Note over Core,Native: native handle 已关闭，callback 已静默，FileAccess 已结束引用
        Core->>Core: 撤销 callback cookie并释放全部 FileAccess backing
        alt Stop 也成功或未调用
            Core-->>Public: Ok
        else Stop 失败
            Core-->>Public: 原始 Stop Err
        end
    else Close 失败
        Core->>Core: 撤销 callback cookie，forget 全部 FileAccess backing
        Core->>Core: finalize_blocked = true，释放 Device 的 RuntimeCore Arc owner
        alt Stop 也失败
            Core-->>Public: DeviceCleanup { stop, close }
        else Stop 成功或未调用
            Core-->>Public: 原始 Close Err
        end
        Note over Core,Native: wrapper 不恢复或重试 handle，不推断 native handle 状态
    end
    Public-->>App: Result；Err 时 run 返回，main 结束进程

    opt Close 与此前业务调用均成功
        App->>Public: sdk.shutdown()，消费 Sdk
        Public->>Core: Runtime::shutdown(self)
        Core->>Core: Arc::try_unwrap(RuntimeCore)
        alt Device 或 ImageProcessor owner 仍存在
            Core-->>Public: InvalidState；不调用 Finalize
            Public-->>App: Err；Sdk 已消费
        else Sdk 是最后一个 owner
            Core->>Core: 检查 finalize_blocked
            alt 曾发生 Close failure
                Core-->>Public: InvalidState；不调用 Finalize
                Public-->>App: Err；Sdk 已消费
            else Finalize 允许
                Core->>Native: MV3D_LP_Finalize()
                Native-->>Core: status
                Core-->>Public: Result
                Public-->>App: Result
            end
        end
    end
```

`Device` 通过 `Arc` 独立拥有 session，打开后可释放 `Sdk`，也可将唯一 device owner 移入普通 worker thread。`get_image()` 使用有限超时，`get_image_blocking()` 传入 SDK 的无限等待值；两者返回拥有 payload 的 `Frame`。`start()` 仅接受 `Idle`，`stop()` 仅接受 `Pulling` 或 `CallbackRunning`；native 调用失败时状态不变。`get_image`、`soft_trigger`、参数、Execute 与 `clear_buffer` 的调用顺序仍由 SDK 判定。

显式 `close()` 与 `Drop` 共用清理路径：运行态先尝试一次 Stop，随后调用一次 Close。Close 返回后才撤销 callback cookie；Close 失败时封存全部 FileAccess backing、置位 `finalize_blocked`，并把 native handle 状态交给进程退出处理。每个 `Device` owner 最多提交一次 Close；显式关闭按单错误原样、双错误聚合的规则返回，Drop 无法返回错误。完整总览见[生命周期与时序图总览](../生命周期与时序图.md)。

`shutdown(self)` 前应先释放全部 `Device` 和 `ImageProcessor`。Arc 唯一性取代 owner 计数，单向 `finalize_blocked` 只记录“曾有 Close 失败”；任一条件不满足都拒绝 Finalize。Finalize status 原样返回且不重试。终止式业务让错误先离开持有局部 owner 的 `run()`，再由 `main()` 记录并结束进程。
