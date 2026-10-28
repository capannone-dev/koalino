<!-- This directory holds the tiny fallback fonts the PDF Unicode tests use. -->

# PDF test fonts

`pdf_unicode_tests.rs` uses these fonts as fallback fonts, so the CJK and emoji tests run the
same way on every machine without depending on installed fonts. They are test fixtures only and
are never compiled into the binary.

| File | Source | Characters |
|---|---|---|
| `NotoSansSC-TestSubset.ttf` | Noto Sans SC (variable, instanced at weight 400) | 测 试 世 界 会 议 记 录 项 目 ⽬ |
| `NotoEmoji-TestSubset.ttf` | Noto Emoji (variable, instanced at weight 400) | 🎉 🌍 |
| `OFL-NotoSansSC.txt`, `OFL-NotoEmoji.txt` | SIL Open Font License 1.1 texts of both fonts | — |

`目` (U+76EE) and the Kangxi radical `⽬` (U+2F6C) share one glyph. That pair exercises the
`ToUnicode` rebuild, which must map the glyph back to the character that was drawn.

## Regenerating

The fonts were cut from the variable fonts in the `google/fonts` repository
(`ofl/notosanssc/NotoSansSC[wght].ttf`, `ofl/notoemoji/NotoEmoji[wght].ttf`) with
[fontTools](https://github.com/fonttools/fonttools), a development tool that is not part of the
build:

```python
from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

font = instancer.instantiateVariableFont(TTFont("NotoSansSC[wght].ttf"), {"wght": 400})
options = subset.Options()
options.name_IDs = ["*"]
options.layout_features = []
options.notdef_outline = True
options.hinting = False
subsetter = subset.Subsetter(options)
subsetter.populate(text="测试世界会议记录项目⽬")
subsetter.subset(font)
font.save("NotoSansSC-TestSubset.ttf")
```

Use the same steps for Noto Emoji with `text="🎉🌍"`. Under the OFL, subsets are modified
versions. Noto Sans SC reserves only the name "Source" and Noto Emoji reserves none, so the subsets
may keep their Noto names. The OFL texts must stay next to the fonts.
