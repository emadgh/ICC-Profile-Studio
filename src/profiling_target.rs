//! Vendor-independent characterization target preparation and measurement QA.
//! This module does not build an ICC or invent colorimetric measurements.
use crate::{R,Profile,lab::MeasuredComparison};
use std::{collections::{BTreeMap,BTreeSet},path::{Path,PathBuf}};
use lcms2::{CIELab,CIELabExt};
const LIMIT:usize=2048;
const PAGE_SIDE:usize=12;
fn check(caps:&[f64],tac:f64,levels:usize)->R<()>{
 if !(2..=15).contains(&caps.len())||caps.iter().any(|x|!x.is_finite()||!(0.0..=100.).contains(x))||!tac.is_finite()||!(0.0..=100.*caps.len() as f64).contains(&tac)||!matches!(levels,3|5|7){return Err("Invalid characterization chart settings".into())}
 Ok(())
}
/// Deterministic bounded design: substrate, single ramps, four mixed
/// levels per pair, all-channel ramps and distributed interior points.
/// Unlike a full factorial design, the sample count is bounded in n.
pub fn recipes(caps:&[f64],tac:f64,levels:usize)->R<Vec<Vec<u16>>>{
 check(caps,tac,levels)?;
 let n=caps.len();let mut seen=BTreeSet::new();let mut out=Vec::new();
 let quant=|v:f64|((v/100.*65535.).floor()) as u16;
 let mut push=|mut v:Vec<f64>|{
  let sum=v.iter().sum::<f64>();
  if sum>tac&&sum>0.{let scale=tac/sum;for x in &mut v{*x*=scale;}}
  let q=v.into_iter().map(quant).collect::<Vec<_>>();
  if out.len()<LIMIT&&seen.insert(q.clone()){out.push(q);}
 };
 push(vec![0.;n]);
 for j in 0..n{for i in 1..levels{let mut v=vec![0.;n];v[j]=caps[j]*i as f64/(levels-1) as f64;push(v);}}
 for j in 0..n{for k in j+1..n{for a in [0.5,1.]{for b in [0.5,1.]{let mut v=vec![0.;n];v[j]=a*caps[j];v[k]=b*caps[k];push(v);}}}}
 for i in 1..levels{push(caps.iter().map(|x|x*i as f64/(levels-1) as f64).collect());}
 let count=match levels{3=>256,5=>512,_=>1024};
 let mut state=0x9e3779b97f4a7c15u64;
 for _ in 0..count{
  let mut v=Vec::with_capacity(n);
  for cap in caps{state^=state<<13;state^=state>>7;state^=state<<17;v.push(cap*((state>>32) as f64/u32::MAX as f64));}
  push(v);
 }
 Ok(out)
}
fn header(n:usize,cmyk:bool)->Vec<String>{
 let mut v=vec!["id".to_owned()];
 if cmyk{v.extend(["c","m","y","k"].map(str::to_owned));}
 else{v.extend((1..=n).map(|i|format!("CH{i}")));}
 v.extend(["L","a","b"].map(str::to_owned));v
}
pub fn export(reference:&Path,root:&Path,levels:usize,caps:&[f64],tac:f64,dpi:u32)->R<PathBuf>{
 let p=Profile::read(reference)?;
 if &p.header[12..16]!=b"prtr"{return Err("Requires a printer ICC profile".into())}
 let n=crate::print_space::format(&p.header[16..20])?.channels;
 if caps.len()!=n{return Err("Ink limits must match physical profile channels".into())}
 if !(50..=1200).contains(&dpi){return Err("Chart DPI must be 50–1200".into())}
 let recipes=recipes(caps,tac,levels)?;
 let patch=(8.*dpi as f64/25.4).round().max(1.) as usize;let gap=(2.*dpi as f64/25.4).round().max(1.) as usize;
 let pitch=patch+gap;
 let side=PAGE_SIDE.min(((24_000_000f64.sqrt() as usize).saturating_sub(gap))/pitch).max(1);
 let per_page=side*side;
 let mut total=0u64;
 for page in recipes.chunks(per_page){let cols=side.min(page.len());let rows=page.len().div_ceil(cols);total=total.checked_add(((cols*pitch+gap) as u64)*((rows*pitch+gap) as u64)*n as u64*2).ok_or("Size overflow")?;}
 if total>1_500_000_000{return Err(format!("Estimated raw TIFF payload {:.0} MB exceeds 1500 MB; reduce DPI or levels",total as f64/1e6))}
 let stage=tempfile::tempdir_in(root).map_err(|e|e.to_string())?;
 let mut csv=csv::Writer::from_path(stage.path().join("measurements.csv")).map_err(|e|e.to_string())?;
 csv.write_record(header(n,p.cmyk())).map_err(|e|e.to_string())?;
 let mut layout=csv::Writer::from_path(stage.path().join("layout.csv")).map_err(|e|e.to_string())?;
 let mut h=vec!["id".to_owned(),"page".into(),"row".into(),"column".into(),"x_px".into(),"y_px".into(),"patch_px".into()];
 h.extend((1..=n).map(|j|format!("CH{j}_u16")));layout.write_record(h).map_err(|e|e.to_string())?;
 for (page_idx,page) in recipes.chunks(per_page).enumerate(){
  let cols=side.min(page.len());let rows=page.len().div_ceil(cols);
  let path=stage.path().join(format!("page_{:03}.tif",page_idx+1));
  let file=std::fs::File::create(path).map_err(|e|e.to_string())?;
  crate::separated_tiff::write(std::io::BufWriter::new(file),(cols*pitch+gap) as u32,(rows*pitch+gap) as u32,n,p.cmyk(),dpi,|y,row|{
   let y=y as usize;if y<gap{return}let r=(y-gap)/pitch;if r>=rows||(y-gap)%pitch>=patch{return}
   for col in 0..cols{if let Some(v)=page.get(r*cols+col){let x=gap+col*pitch;for pixel in row[x*n..(x+patch)*n].chunks_exact_mut(n){pixel.copy_from_slice(v);}}}
  })?;
  for (i,v) in page.iter().enumerate(){
   let id=format!("P{:05}",page_idx*per_page+i+1);
   let mut rec=vec![id.clone()];rec.extend(v.iter().map(|&q|format!("{:.12}",100.*f64::from(q)/65535.)));rec.extend([String::new(),String::new(),String::new()]);csv.write_record(rec).map_err(|e|e.to_string())?;
   let (r,c)=(i/cols,i%cols);
   let mut rec=vec![id,(page_idx+1).to_string(),(r+1).to_string(),(c+1).to_string(),(gap+c*pitch).to_string(),(gap+r*pitch).to_string(),patch.to_string()];
   rec.extend(v.iter().map(u16::to_string));layout.write_record(rec).map_err(|e|e.to_string())?;
  }
 }
 csv.flush().map_err(|e|e.to_string())?;layout.flush().map_err(|e|e.to_string())?;drop(csv);drop(layout);
 std::fs::write(stage.path().join("README.txt"),format!("{} unique patches; {} channels; {} pages; {} DPI. 8 mm patches, 2 mm gaps. Design: single ramps, pair mixtures, common ramps, deterministic distributed samples. This is a bounded characterization design, not a full factorial or a guaranteed sufficient profiling target. Channel order is physical ICC order. TIFFs are separated 16-bit, untagged; verify safe ink limits and RIP mapping, disable color conversion. Fill only L,a,b with actual absolute D50/2-degree measurements; record measurement mode, backing, substrate and instrument separately. Do not change IDs or recipes. layout.csv preserves exact unsigned 16-bit codes and 1-based page/row/column. No ICC is generated.",recipes.len(),n,recipes.len().div_ceil(per_page),dpi)).map_err(|e|e.to_string())?;
 let dest=root.join("profiling_chart");
 if dest.exists(){return Err("profiling_chart folder already exists".into())}
 std::fs::rename(stage.path(),&dest).map_err(|e|e.to_string())?;Ok(dest)
}
#[derive(Debug)]
struct Record{ink:Vec<f64>,lab:[f64;3]}
fn measured(path:&Path)->R<(Vec<String>,BTreeMap<String,Record>)>{
 if std::fs::metadata(path).map_err(|e|e.to_string())?.len()>32*1024*1024{return Err("CSV exceeds 32 MiB".into())}
 let mut r=csv::Reader::from_path(path).map_err(|e|e.to_string())?;
 let h=r.headers().map_err(|e|e.to_string())?.iter().map(str::to_owned).collect::<Vec<_>>();
 let n=h.len().checked_sub(4).ok_or("Invalid measurement CSV header")?;
 if !(2..=15).contains(&n)||!(h==header(n,false)||n==4&&h==header(4,true)){return Err("Expected id, physical channel fields, L,a,b".into())}
 let mut data=BTreeMap::new();
 for row in r.records(){let row=row.map_err(|e|e.to_string())?;
  if data.len()>=100000||row.len()!=n+4||row[0].trim().is_empty(){return Err("Invalid measurement CSV row or patch limit".into())}
  let vals=row.iter().skip(1).map(|v|v.trim().parse::<f64>().map_err(|_|"Missing or invalid measured number".to_owned())).collect::<R<Vec<_>>>()?;
  if vals.iter().any(|v|!v.is_finite())||vals[..n].iter().any(|v|!(0.0..=100.).contains(v))||!(0.0..=100.).contains(&vals[n])||vals[n+1..].iter().any(|v|!(-128.0..=127.).contains(v)){return Err("Measurement outside supported range".into())}
  if data.insert(row[0].to_owned(),Record{ink:vals[..n].to_vec(),lab:[vals[n],vals[n+1],vals[n+2]]}).is_some(){return Err("Duplicate sample ID".into())}
 }
 if data.is_empty(){return Err("No measured patches".into())}Ok((h,data))
}
/// Requires explicit operator confirmation of comparable D50/2 conditions.
pub fn compare(a:&Path,b:&Path)->R<MeasuredComparison>{
 let (ha,a)=measured(a)?;let (hb,b)=measured(b)?;
 if ha!=hb||a.keys().ne(b.keys()){return Err("Measurement channel columns and patch IDs must match".into())}
 let mut ds=Vec::new();let mut bias=[0.;3];let mut worst=String::new();let mut max=-1.;
 let lab=|v:[f64;3]|CIELab{L:v[0],a:v[1],b:v[2]};
 for (id,x) in &a{
  let y=&b[id];
  if x.ink.iter().zip(&y.ink).any(|(a,b)|(a-b).abs()>0.0001){return Err(format!("Ink recipe differs at patch {id}"))}
  let d=lab(x.lab).cie2000_delta_e(&lab(y.lab),1.,1.,1.);
  if !d.is_finite()||d<0.{return Err("Invalid measured Delta E".into())}
  if d>max{max=d;worst=id.clone();}ds.push(d);
  for j in 0..3{bias[j]+=y.lab[j]-x.lab[j];}
 }
 ds.sort_by(f64::total_cmp);let count=ds.len();
 for v in &mut bias{*v/=count as f64;}
 Ok(MeasuredComparison{count,mean:ds.iter().sum::<f64>()/count as f64,p95:ds[((count as f64*0.95).ceil() as usize)-1],max,worst,bias})
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn bounded_design_all_channel_counts(){
  for n in 2..=15{let mut caps=vec![100.;n];caps[n-1]=0.;let a=recipes(&caps,175.,7).unwrap();assert_eq!(a,recipes(&caps,175.,7).unwrap());assert!(a.len()<=LIMIT);assert_eq!(a[0],vec![0;n]);assert_eq!(a.len(),a.iter().collect::<BTreeSet<_>>().len());
   assert!(a.iter().all(|v|v.len()==n&&v[n-1]==0&&v.iter().map(|q|100.*f64::from(*q)/65535.).sum::<f64>()<=175.+1e-9));}
 }
 #[test]fn bundle_6clr_and_empty_measurements(){
  let d=tempfile::tempdir().unwrap();let p=d.path().join("source.icc");crate::print_space::fixture(6).save_new(&p).unwrap();
  let dest=export(&p,d.path(),3,&[100.,100.,100.,0.,0.,0.],160.,50).unwrap();
  assert!(dest.join("page_001.tif").exists());assert!(dest.join("layout.csv").exists());
  let m=dest.join("measurements.csv");let s=std::fs::read_to_string(&m).unwrap();assert!(s.starts_with("id,CH1,CH2,CH3,CH4,CH5,CH6,L,a,b"));assert!(measured(&m).is_err());
  assert!(export(&p,d.path(),3,&[100.;6],160.,50).is_err());
 }
 #[test]fn dynamic_measured_pair(){
  let d=tempfile::tempdir().unwrap();let a=d.path().join("a.csv");let b=d.path().join("b.csv");
  let h="id,CH1,CH2,CH3,CH4,CH5,CH6,L,a,b\n";
  std::fs::write(&a,format!("{h}p,1,2,3,4,5,6,50,2.6772,-79.7751\n")).unwrap();
  std::fs::write(&b,format!("{h}p,1,2,3,4,5,6,50,0,-82.7485\n")).unwrap();
  assert!((compare(&a,&b).unwrap().mean-2.0425).abs()<0.0001);
  std::fs::write(&b,format!("{h}p,1,2,3,4,5,7,50,0,-82.7485\n")).unwrap();assert!(compare(&a,&b).is_err());
 }
}
