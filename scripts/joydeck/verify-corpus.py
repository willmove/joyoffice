"""Independent python-pptx checks over every authored qualification document."""
import argparse,hashlib,json,zipfile
from pathlib import Path
from pptx import Presentation
def main():
 p=argparse.ArgumentParser();p.add_argument('directory',type=Path);a=p.parse_args();root=a.directory
 manifest=json.loads((root/'manifest.json').read_text());assert manifest['count']==128 and manifest['externalDocuments']==0
 summaries=[]
 for row in manifest['documents']:
  native=root/(row['id']+'.joydeck');public=root/(row['id']+'.pptx')
  assert hashlib.sha256(native.read_bytes()).hexdigest()==row['nativeSha256']
  assert hashlib.sha256(public.read_bytes()).hexdigest()==row['publicPptxSha256']
  deck=Presentation(public);assert len(deck.slides)==row['pages']
  assert deck.slides[0].shapes[0].text==row['firstTitle']
  charts=[s.chart for slide in deck.slides for s in slide.shapes if s.has_chart];assert len(charts)==7
  assert list(charts[0].series[0].values)==row['firstChartValues'],row['id']
  if row['family']=='table':assert any(s.has_table for s in deck.slides[0].shapes)
  if row['family']=='image-crop':assert any(s.shape_type==13 for s in deck.slides[0].shapes)
  assert row['origin']=='project-authored' and row['license']=='MIT OR Apache-2.0'
  summaries.append({'id':row['id'],'charts':len(charts),'passed':True})
 result={'publicReader':'python-pptx','documents':len(summaries),'pages':sum(r['pages'] for r in manifest['documents']),'charts':sum(r['charts'] for r in summaries),'families':sorted({r['family'] for r in manifest['documents']}),'externalDocuments':0,'passed':True,'scope':'public structure, text/data, hashes and declared losses; no independent Office visual certification'}
 (root/'independent-verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
