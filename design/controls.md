# 控件规格（阶段 1 产物）

> 由 `tools/extract-tokens.mjs` 从 DSH-NEXT `app.asar` 自动生成，请勿手改。
> 基线 asar：`<install>/resources/app.asar`
> asar sha256：`16f2c62b1ccc98a968d9c7cf5edcd7af7a1e7690c2fba8cea361d1726d0db349`

## 1. 圆角刻度

| 档位 | token | 值 | 来源 |
| --- | --- | --- | --- |
| xs | `--dsw-radius-xs` | 4px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:base.css |
| sm | `--dsw-radius-sm` | 8px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:base.css |
| md | `--dsw-radius-md` | 12px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:base.css |
| lg | `--dsw-radius-lg` | 16px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:base.css |
| xl | `--dsw-radius-xl` | 20px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:base.css |
| panel | `--dsw-radius-panel` | 28px | lib/native-ui/assets/index-BstoNZLX.css:index-BstoNZLX.css |

## 2. 控件规格

| 用途 | token | light 值 | 说明 | 来源 |
| --- | --- | --- | --- | --- |
| 卡片圆角 | `--dsw-radius-lg` | 16px | 设置页卡片圆角 16px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:base.css |
| 行圆角 | `--dsw-radius-md` | 12px | 列表行 / 输入框圆角 12px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:base.css |
| 面板圆角 | `--dsw-radius-panel` | 28px | 浮层 / 面板圆角 28px | lib/native-ui/assets/index-BstoNZLX.css:index-BstoNZLX.css |
| 小圆角 | `--dsw-radius-xs` | 4px | 徽标 / 标签圆角 4px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:base.css |

## 3. 实测尺寸（primitives CSS）

- `Menu.module.css` 高度：12px / 14px / 15px / 16px / 17px / 20px / 24px / 30px / 34px
- `Menu.module.css` 圆角：`var`
- `HoverCard.module.css` 高度：1px / 20px
- `HoverCard.module.css` 圆角：`var`

## 4. 复用片段（design/parts）

| 文件 | 字节 | sha256 | 说明 |
| --- | --- | --- | --- |
| scrollbar.css | 838 | `7457fa7bcde76d19981125c94c0a8afd7d83b1dac3f72bd418597bda35297c5b` | 原生滚动条 |
| focus.css | 298 | `77bcbea3e42c33667afa8e835bf798cc9bbc4bdcd0d37e796423af9dbd9bbded` | 焦点环 |
| corner-shape.css | 141 | `b3ef70d30ee48289413e753be96b56a1e3273115d8123cc22105504dbd38ebe7` | 超椭圆圆角 |
| Menu.module.css | 6491 | `77265cc046078c060ca2f5b700b31c77f024a32e1834ebd3a0fc8b4562c7134a` | 菜单表面 |
| HoverCard.module.css | 1963 | `03e81485eb927d73f3db0352e8910c9bd6ce76fb8a7e5773ca16b2a61f151dfa` | 悬浮卡表面 |

## 5. 抽不到的项（显式记录，不静默丢弃）

- 无
