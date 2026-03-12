---
name: excel-processor
description: Excel 表格处理工具
kind: tool
---

# Excel 处理器

处理 Excel 表格，支持读取、写入、数据分析。

## 使用方法

```python
import pandas as pd

# 读取 Excel
df = pd.read_excel('data.xlsx')

# 数据分析
print(df.describe())
print(df.head())

# 写入 Excel
df.to_excel('output.xlsx', index=False)
```

## 功能

- 读取 Excel 文件（.xlsx, .xls）
- 数据清洗和转换
- 统计分析
- 生成新的 Excel 文件
- 支持多个工作表
