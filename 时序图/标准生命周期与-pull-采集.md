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
        Core->>Core: Device 获取 RuntimeCore 的 Arc owner，measuring = false
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
    Core->>Native: MV3D_LP_StartMeasure(handle)
    Native-->>Core: status
    alt StartMeasure 成功
        Core->>Core: measuring = true
        Core-->>Public: Ok
        Public-->>App: Ok
    else StartMeasure 失败
        Core-->>Public: Err，measuring 仍为 false
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
        Core->>Core: measuring = false
        Core-->>Public: Ok
        Public-->>App: Ok
    else Stop 失败
        Core-->>Public: Err，measuring 仍为 true
        Public-->>App: Err；可再次 stop 或关闭
    end

    App->>Public: device.close()
    Public->>Core: Device::close() 消费 owner 并取走 handle
    opt measuring = true
        Core->>Native: MV3D_LP_StopMeasure(handle) 一次
        Native-->>Core: stop status
    end
    Core->>Native: MV3D_LP_CloseDevice(handle) 一次
    Native-->>Core: close status
    Note over Core,Native: 任意 close status 都使 handle 失效；callback 已静默，FileAccess 已结束引用
    Core->>Core: 撤销 callback cookie，释放 FileAccess 文件名与 RuntimeCore Arc owner
    Core-->>Public: 汇总 Stop 与 Close 结果
    Public-->>App: Result
    Note over Core,Native: 错误 status 也不恢复 handle，随后 Drop 不再调用 Close

    App->>Public: sdk.shutdown()，消费 Sdk
    Public->>Core: Runtime::shutdown(self)
    Core->>Core: Arc::try_unwrap(RuntimeCore)
    alt Device 或 ImageProcessor owner 仍存在
        Core-->>Public: InvalidState；不调用 Finalize
        Public-->>App: Err；Sdk 已消费，本进程不重试
    else Sdk 是最后一个 owner
        Core->>Native: MV3D_LP_Finalize()
        Native-->>Core: status
        Core-->>Public: Result
        Public-->>App: Result
    end
```

`Device` 通过 `Arc` 独立拥有 session，打开后可释放 `Sdk`，也可将唯一 device owner 移入普通 worker thread。`get_image()` 使用有限超时，`get_image_blocking()` 传入 SDK 的无限等待值；两者返回拥有 payload 的 `Frame`。wrapper 只用 `measuring` 标记处理 Start、Stop 和关闭时的必要 Stop，`get_image`、`soft_trigger`、参数、Execute 与 `clear_buffer` 的调用顺序由 SDK 判定。

显式 `close()` 与 `Drop` 共用清理路径：测量中尝试一次 Stop，随后调用一次 Close。wrapper 依赖 Close 返回时 handle 已永久失效、native callback 已静默且 FileAccess 已停止引用文件名；这些条件同样适用于错误 status。每个 `Device` owner 最多提交一次 Close；显式关闭返回清理错误，Drop 忽略错误。完整总览见[生命周期与时序图总览](../生命周期与时序图.md)。

`shutdown(self)` 前应先释放全部 `Device` 和 `ImageProcessor`。Arc 唯一性直接取代 session 状态机与 owner 计数；Finalize 无论成功或失败都不重试。业务若采用 abort 策略，应在持有这些局部 owner 的 `run()` 返回后再终止进程，使普通错误路径先完成 Rust 清理。
