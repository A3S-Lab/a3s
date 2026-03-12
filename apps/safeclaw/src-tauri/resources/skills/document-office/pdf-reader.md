---
name: pdf-reader
description: 读取和提取 PDF 文档内容
kind: tool
---

# PDF 阅读器

读取 PDF 文件并提取文本内容。

## 使用方法

```bash
# 读取 PDF 文件
pdftotext input.pdf output.txt

# 或使用 Python
python -c "
import PyPDF2
with open('input.pdf', 'rb') as f:
    reader = PyPDF2.PdfReader(f)
    for page in reader.pages:
        print(page.extract_text())
"
```

## 功能

- 提取文本内容
- 保留基本格式
- 支持多页文档
