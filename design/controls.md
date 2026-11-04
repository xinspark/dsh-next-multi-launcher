# 控件规格（阶段 1 产物）

> 由 `tools/extract-tokens.mjs` 从 DSH-NEXT `app.asar` 自动生成，请勿手改。
> 基线 asar：`E:\FXS\APP\DSH NEXT\resources\app.asar`
> asar sha256：`c7057e19e777ff6423815268901ecfe30382323e0dc2e5405b7a7e915328607e`

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
| scrollbar.css | 860 | `e2b2b6f894032119b17d131e1924056530188eab04960159739ee4faa641a1c5` | 原生滚动条 |
| focus.css | 298 | `77bcbea3e42c33667afa8e835bf798cc9bbc4bdcd0d37e796423af9dbd9bbded` | 焦点环 |
| corner-shape.css | 141 | `b3ef70d30ee48289413e753be96b56a1e3273115d8123cc22105504dbd38ebe7` | 超椭圆圆角 |
| Menu.module.css | 8046 | `832c783823216c61adeebec7c120efc71467d3ca7f829137c03e84ae71446f45` | 菜单表面 |
| HoverCard.module.css | 2084 | `e5dc1c4235065942925aecd48b36c29c8f4df79ec571345b95fdd7f2fef82c02` | 悬浮卡表面 |

## 5. 抽不到的项（显式记录，不静默丢弃）

- 无
