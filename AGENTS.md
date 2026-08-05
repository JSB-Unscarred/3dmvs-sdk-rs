<!--  3dmvs-sdk-rs -->
# 项目功能

- 本项目是对海康威视激光轮廓仪的3DMVS SDK的Rust安全封装。

# 语言

- 主体用中文，但是英文术语不需要翻译成中文。
- 保持简洁，不要使用“没有xxx”的句式。

# 编码规则

- 一切从简，不要Overdesign，优先给出能实现安全包装的最小设计。
- 安全与否需要你根据厂商的文档、示例程序和头文件等进行适当的推导，厂商的SDK的安全性足以用于生产环境。
- 不要引入过度复杂的安全设计，先专注于实现功能；每个安全设计都要在方案和注释中精简地给出理由（防止什么问题）。
- 函数、模块都要用注释描述其功能，说明为了防止什么问题，引入了什么安全设计；但要保持简洁，特别是不记录修改历史。
- 测试必须要精简，必要测试要注释说明针对的功能或约定。
- 修改代码后要同步更新注释和测试。

# Git

- 修改代码后要给出详细的commit message，使用英文前缀加中文说明，描述修改的内容；正文记录对每个部分的修改。

# README文档

- 维护一个SDK接口对应的安全Rust接口定义表格
- 维护一个SDK结构体对应的Rust结构体定义表格
- 本项目暂时不会发布到Crates.io

# 索引

- 生命周期与时序图总览：[生命周期与时序图.md](生命周期与时序图.md)
- 标准生命周期与 pull 采集：[标准生命周期与-pull-采集.md](时序图/标准生命周期与-pull-采集.md)
- callback 采集与停止：[callback-采集与停止.md](时序图/callback-采集与停止.md)
- 文件上传与下载：[文件上传与下载.md](时序图/文件上传与下载.md)
- SDK的环境变量： MV3DLP_DEV_ENV
- SDK说明文档：C:\Program Files (x86)\3DMVS\Development\Documentations\3D激光轮廓传感器SDK开发指南V1.3.2（C）.chm
- SDK的头文件目录：C:\Program Files (x86)\3DMVS\Development\Includes
- SDK的示例程序目录：C:\Program Files (x86)\3DMVS\Development\Samples\C
