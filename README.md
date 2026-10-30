# FE 六曾云端评测

手动启动 Evaluate workflow：一个 Windows job 编译，六个 Windows job 各测 50 个独立种子，每台 4 个 worker，最后严格核验并汇总 300 条完整失败链。初始阳光 100000、上限 2147483647。成功关净消耗按所有成功关加权计算；失败原因为每条链的终局原因。

`SNAPSHOT.json` 记录原始策略、框架和模拟器提交。仅包含所需源码快照，没有原仓库历史或游戏素材。`requests/` 保存每批独立种子；不同打法不得使用相同种子。默认不输出详细审计和完整回放。

工作流仅由 workflow_dispatch 启动，同一批失败重试复用该批种子，后续新批使用新的 request。每批独立 artifact，保留 7 天；汇总中任何缺失、重复、截断或版本混用都判为失败，不计算成通关。需要长期保存的结果请下载。


2026-10-30 的完整测量与版本链接见 [对比结果](reports/2026-10-30/comparison.json)。当前保留实验候选为 `experiment/02-sustained-contact`；第三组保留在 `experiment/03-sustained-contact`，尚无额外收益证据。原生啃食首版出现的短暂接触回退与修复见 [诊断记录](reports/2026-10-30/exact-contact-regression.json)。主分支保存评测基础设施与原始云端基线，策略实验由对应分支独立管理。
