# 控件规格（阶段 1 产物）

> 由 `tools/extract-tokens.mjs` 从 DSH-NEXT `app.asar` 自动生成，请勿手改。
> 基线 asar：`<install>/resources/app.asar`
> asar sha256：`3bccf05a97aeb6bed3c3cc7ebe958533c0e6e4afa6c902d274776b572411aefa`

## 1. 圆角刻度

| 档位 | token | 值 | 来源 |
| --- | --- | --- | --- |
| xs | `--dsw-radius-xs` | 4px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:inline-css |
| sm | `--dsw-radius-sm` | 8px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:inline-css |
| md | `--dsw-radius-md` | 12px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:inline-css |
| lg | `--dsw-radius-lg` | 16px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:inline-css |
| xl | `--dsw-radius-xl` | 20px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:inline-css |
| panel | `--dsw-radius-panel` | 28px | @deepseek-ai/dsh-client-ui-theme/src/styles/base.css:base.css |

## 2. 控件规格

| 用途 | token | light 值 | 说明 | 来源 |
| --- | --- | --- | --- | --- |
| 卡片圆角 | `--dsw-radius-lg` | 16px | 设置页卡片圆角 16px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:inline-css |
| 行圆角 | `--dsw-radius-md` | 12px | 列表行 / 输入框圆角 12px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:inline-css |
| 面板圆角 | `--dsw-radius-panel` | 28px | 浮层 / 面板圆角 28px | @deepseek-ai/dsh-client-ui-theme/src/styles/base.css:base.css |
| 小圆角 | `--dsw-radius-xs` | 4px | 徽标 / 标签圆角 4px | @deepseek-ai/dsh-client-ui-theme/lib/client.js:inline-css |

## 3. 实测尺寸（primitives CSS）

- `Menu.module.css` 高度：12px / 14px / 15px / 16px / 17px / 20px / 24px / 30px / 34px
- `Menu.module.css` 圆角：`var`
- `HoverCard.module.css` 高度：1px / 20px
- `HoverCard.module.css` 圆角：`var`

## 4. 复用片段（design/parts）

| 文件 | 字节 | sha256 | 说明 |
| --- | --- | --- | --- |
| Menu.module.css | 11691 | `ac03c6b4777b1b86e0e2ae09591fd2fe3c0da5718c493b458f67cbc7990b7649` | 菜单表面 |
| HoverCard.module.css | 2318 | `03853003f185ef3856f27a4822f178bc7d2b032dea3da922746c03ed2f423b22` | 悬浮卡表面 |

## 5. 抽不到的项（显式记录，不静默丢弃）

- `node_modules/@deepseek-ai/dsh-client-ui-theme/lib/client.js`：未找到内联片段 scrollbar
- `node_modules/@deepseek-ai/dsh-client-ui-theme/lib/client.js`：未找到内联片段 focus
- `node_modules/@deepseek-ai/dsh-client-ui-theme/lib/client.js`：未找到内联片段 corner-shape
