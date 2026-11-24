# Bulgarian and Galician source archive

The two runtime archives are the original release bytes. The per-locale
directories in this bundle contain the fixed spelling data, TeX patterns and
notices. The author archives under `upstream/` supply the editable materials.

## Bulgarian (`bg`)

`bg-4.1.zip` is the author's OpenOffice spelling package and contains its GPL
version 2 notices and documentation. `bgoffice_4.1.orig.tar.gz` is the author
source archived by Debian. It includes the editable word-class database and
build materials. To reproduce the fixed word list and rule file, decode
`bg_BG.dic` and `bg_BG.aff` from Windows-1251, replace CRLF with LF and encode
as UTF-8. Change the affix's first line from `SET microsoft-cp1251` to
`SET UTF-8`. The Bulgarian TeX pattern has its own notice in
`LICENCES/hyph-bg.tex`.

## Galician (`gl`)

`gl-20.08.tar.gz` contains the author modules, SCons build definitions, and the
English and Galician GPL version 3 notices. Build in the extracted directory
with a UTF-8 locale and the dependencies listed by the author.

```sh
scons code=gl_ES aff=norma,trasno,unidades \
  dic=ceg,comunidade,iso639,iso4217,microsoft,norma,rag,santiago,trasno,udc,unidades,usc,uvigo,volga,wikipedia,wiktionary \
  rep=comunidade,rag,wikipedia
```

The fixed LibreOffice 20.08 derivative moves the complete `SFX 1011` group
after the `PFX 1004` group. With Windows ICU the only dictionary order
differences are the relative positions of the ASCII and curly apostrophe
variants in the `Rus` and `Tiananmen` groups. The fixed dictionary orders the
ASCII apostrophe variants first. All 232,425 dictionary lines and all affix
lines, including their multiplicities, match after line-ending normalization.
The source bundle includes the fixed-order outputs. The TeX pattern is
distributed under the Unlicense.
