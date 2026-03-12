---
name: word-editor
description: Word 文档编辑工具
kind: tool
---

# Word 编辑器

创建和编辑 Word 文档。

## 使用方法

```python
from docx import Document

# 创建新文档
doc = Document()
doc.add_heading('文档标题', 0)
doc.add_paragraph('这是一段文字')
doc.save('output.docx')

# 读取文档
doc = Document('input.docx')
for para in doc.paragraphs:
    print(para.text)
```

## 功能

- 创建新文档
- 添加标题、段落、表格
- 设置字体和样式
- 插入图片
- 保存为 .docx 格式
