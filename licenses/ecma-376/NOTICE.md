# ECMA-376 DrawingML preset geometry

© 2016 Ecma International. JoyDeck uses the unmodified machine-readable preset definitions from ECMA-376 Part 1, fifth edition (December 2016), for the Office Open XML shapes defined in §20.1.9. The Rust compiler, guide evaluation, path construction and pointer handling are implemented in JoyOffice. The accompanying Ecma policy contains the copyright license and disclaimer applicable to the standard. [Standard](https://ecma-international.org/publications-and-standards/standards/ecma-376/) · [Ecma license and disclaimer](Ecma_text_copyright_policy_version_1.pdf)

| Distribution item | Identity |
| --- | --- |
| Original archive | [ECMA-376 Part 1, fifth edition](https://ecma-international.org/wp-content/uploads/ECMA-376-1_5th_edition_december_2016.zip) |
| Archive member | `OfficeOpenXML-DrawingMLGeometries.zip` → `presetShapeDefinitions.xml` |
| Vendored source | [presetShapeDefinitions.xml](../../crates/joydeck-geom/data/presetShapeDefinitions.xml) |
| SHA-256 of original XML, including BOM and CRLF | `2f7c868d857c1e3c4b5a6068759fe0e07d77ad58377a6618d1b02ba3507b6939` |

The attachment includes two entries named `upDownArrow` and eight three-operand guide expressions with a trailing zero. The compiler accepts the documented trailing argument. JoyDeck retains its separate, tested generators for the single and opposed up arrows.
