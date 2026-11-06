import importlib.util
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('complex_corpus', Path(__file__).parents[1] / 'joyword/check-complex-corpus.py')
corpus = importlib.util.module_from_spec(spec)
spec.loader.exec_module(corpus)


class ComplexCorpus(unittest.TestCase):
    def test_reproducible_fixtures_and_loss_detection(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for folder in ('first', 'second'):
                (root / folder).mkdir()
                corpus.generate(root / folder)
            for kind in ('notes', 'equation', 'ole'):
                source = root / 'first' / f'{kind}.docx'
                self.assertEqual(corpus.digest(source), corpus.digest(root / 'second' / source.name))
                original = corpus.invariant(source, kind)
                with zipfile.ZipFile(source) as z:
                    parts = {n: z.read(n) for n in z.namelist()}
                if kind == 'notes':
                    parts['word/footnotes.xml'] = parts['word/footnotes.xml'].replace(b'Synthetic footnote', b'LOST')
                elif kind == 'equation':
                    parts['word/document.xml'] = parts['word/document.xml'].replace(b'<m:t>y</m:t>', b'<m:t>LOST</m:t>')
                else:
                    parts['word/embeddings/payload.bin'] = b'LOST'
                changed = root / f'{kind}-damaged.docx'
                with zipfile.ZipFile(changed, 'w') as z:
                    for name, data in parts.items():
                        z.writestr(name, data)
                if kind == 'ole':
                    with self.assertRaisesRegex(ValueError, 'payload changed'):
                        corpus.invariant(changed, kind)
                else:
                    self.assertNotEqual(corpus.invariant(changed, kind), original)

    def test_chart_and_smartart_graph_corruption_is_detected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            corpus.generate(root)
            for kind, payload in [('chart', 'word/embeddings/workbook.xlsx'),
                                  ('smartart', 'word/diagrams/layout1.xml')]:
                source = root / f'{kind}.docx'
                expected = corpus.invariant(source, kind)
                with zipfile.ZipFile(source) as z:
                    original = {n: z.read(n) for n in z.namelist()}
                for mutation in ('missing', 'changed', 'content-type'):
                    parts = dict(original)
                    if mutation == 'missing':
                        del parts[payload]
                    elif mutation == 'changed':
                        parts[payload] += b' changed'
                    else:
                        parts['[Content_Types].xml'] = parts['[Content_Types].xml'].replace(
                            b'application/vnd.openxmlformats-officedocument.', b'application/synthetic-wrong.')
                    output = root / f'{kind}-{mutation}.docx'
                    with zipfile.ZipFile(output, 'w') as z:
                        for name, data in parts.items():
                            z.writestr(name, data)
                    if mutation == 'missing':
                        with self.assertRaises(KeyError):
                            corpus.invariant(output, kind)
                    else:
                        self.assertNotEqual(corpus.invariant(output, kind), expected)


if __name__ == '__main__':
    unittest.main()
