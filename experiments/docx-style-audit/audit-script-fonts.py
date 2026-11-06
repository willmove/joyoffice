"""Count retained EA/CS references in the nine-corpus probe outputs; no font resolution guesses."""
import argparse, collections, hashlib, json, pathlib, zipfile, xml.etree.ElementTree as ET
p=argparse.ArgumentParser()
p.add_argument('--probe', type=pathlib.Path, required=True)
p.add_argument('--output', type=pathlib.Path, required=True)
a=p.parse_args()
root=pathlib.Path(__file__).resolve().parents[2]
w='{http://schemas.openxmlformats.org/wordprocessingml/2006/main}'
keys=['eastAsiaTheme','cstheme','eastAsia','cs','hint']
def refs(path):
    counts={k:collections.Counter() for k in keys}
    with zipfile.ZipFile(path) as z:
        for name in ['word/styles.xml','word/document.xml']:
            if name not in z.namelist(): continue
            tree=ET.fromstring(z.read(name))
            for el in tree.iter(w+'rFonts'):
                for k in keys:
                    if w+k in el.attrib: counts[k][el.attrib[w+k]]+=1
    return {k:dict(v) for k,v in counts.items()}
rows=[]
for i,row in enumerate(json.loads((a.probe/'theme-response.json').read_text())):
    src=root/row['path']; saved=a.probe/'saved'/f'{i:02}.docx'
    rows.append({'path':row['path'],'source_sha256':hashlib.sha256(src.read_bytes()).hexdigest(),
                 'original':refs(src),'saved':refs(saved)})
a.output.write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n')
for key in keys:
    print(key, sum(sum(r['original'][key].values()) for r in rows),sum(sum(r['saved'][key].values()) for r in rows))
