# 01 · asar 读取

## 症状

- 读取 `app.asar` 索引时，条目的 `offset` / `size` 偶尔解析成 `NaN`；
- 从 `resources/app.asar` 里读 `package.json` 报"文件不存在"，但资源管理器里能看到。

## 根因

两条独立问题：

1. asar 索引里的 `offset` / `size` **可能是 JSON 字符串**（`"12345"`）而不是数字；
2. 部分条目带 `"unpacked": true`，数据**不在** asar 主体里，而在 `<asar>.unpacked/` 目录下。

## 证据

```text
# 索引片段（截断）
{ "files": { "package.json": { "size": "847", "offset": "0" } } }
```

第一次实现只按数字解析，`Number("0")` 没问题但 `offset` 是相对数据区的偏移；
遇到 `"unpacked": true` 时按主体偏移读会读到别的内容，或直接越界。

## 结论

- `json_u64` 同时接受 `number` 与 `string`；
- 遇到 `unpacked` 条目，从 `<asar路径>.unpacked/<条目路径>` 读；
- 主体数据区起点 = `8 + pickleLen`（`pickleLen` 也需按字符串/数字两种解析）。

对应实现：`src-tauri/src/asar.rs`、`tools/asar-lib.mjs`。
