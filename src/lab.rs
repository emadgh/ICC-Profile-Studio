//! Ceramic profiling workflows: direct-device charts, measured comparisons,
 //! and official Argyll colprof integration. No synthetic measurements.
use std::{fs,path::{Path,PathBuf},collections::BTreeMap};
use lcms2::{CIELab,CIELabExt};
type R<T>=Result<T,String>;
const HEADER:&str="id,c,m,y,k,L,a,b";
#[derive(Clone,Debug)]
pub struct Measurement{pub id:String,pub inks:[f64;4],pub lab:[f64;3]}
#[derive(Clone,Debug)]
pub struct MeasuredComparison{pub count:usize,pub mean:f64,pub p95:f64,pub max:f64,pub worst:String,pub bias:[f64;3]}
pub fn load_csv(p:&Path)->R<BTreeMap<String,Measurement>>{
 if fs::metadata(p).map_err(|e|e.to_string())?.len()>16*1024*1024{return Err("Measurement CSV exceeds 16 MiB".into())}
 let text=fs::read_to_string(p).map_err(|e|e.to_string())?;
 let mut lines=text.trim_start_matches('\u{feff}').lines();
 if lines.next().map(str::trim)!=Some(HEADER){return Err(format!("Expected CSV header {HEADER}"))}
 let mut out=BTreeMap::new();
 for (i,line) in lines.enumerate(){
  if line.trim().is_empty(){continue}
  let f:Vec<_>=line.split(',').map(str::trim).collect();
  if f.len()!=8||f[0].is_empty(){return Err(format!("Invalid row {}",i+2))}
  let mut n=[0f64;7];for j in 0..7{n[j]=f[j+1].parse().map_err(|_|format!("Missing/invalid measured value in row {}",i+2))?;}
  if n.iter().any(|v|!v.is_finite())||n[..4].iter().any(|v|!(0.0..=100.).contains(v))||!(0.0..=100.).contains(&n[4])||n[5..].iter().any(|v|!(-128.0..=127.).contains(v)){return Err(format!("Invalid range at row {}",i+2))}
  let m=Measurement{id:f[0].into(),inks:[n[0],n[1],n[2],n[3]],lab:[n[4],n[5],n[6]]};
  if out.insert(m.id.clone(),m).is_some(){return Err(format!("Duplicate patch {}",f[0]))}
 }
 if out.is_empty(){return Err("No measured patches".into())}Ok(out)
}
pub fn compare(a:&Path,b:&Path)->R<MeasuredComparison>{
 let a=load_csv(a)?;let b=load_csv(b)?;
 if a.len()!=b.len()||a.keys().ne(b.keys()){return Err("Measured files must contain the same patch IDs".into())}
 let mut values=Vec::new();let mut worst=String::new();let mut max=-1.;let mut bias=[0.;3];
 for (id,x) in &a{
  let y=&b[id];if x.inks.iter().zip(y.inks).any(|(&a,b)|(a-b).abs()>0.0001){return Err(format!("Ink recipe differs at patch {id}"))}
  let p=CIELab{L:x.lab[0],a:x.lab[1],b:x.lab[2]};let q=CIELab{L:y.lab[0],a:y.lab[1],b:y.lab[2]};
  let de=p.cie2000_delta_e(&q,1.,1.,1.);if !de.is_finite()||de<0.{return Err("Invalid measured Delta E".into())}
  values.push(de);if de>max{max=de;worst=id.clone();}for j in 0..3{bias[j]+=y.lab[j]-x.lab[j];}
 }
 let count=values.len();for v in &mut bias{*v/=count as f64;}values.sort_by(f64::total_cmp);
 Ok(MeasuredComparison{count,mean:values.iter().sum::<f64>()/count as f64,p95:values[((count as f64*0.95).ceil() as usize)-1],max,worst,bias})
}
pub fn chart_recipes(levels:usize,caps:[f64;4],tac:f64)->R<Vec<[u16;4]>>{
 if !(3..=7).contains(&levels)||!tac.is_finite()||!(0.0..=400.).contains(&tac)||caps.iter().any(|v|!v.is_finite()||!(0.0..=100.).contains(v)){return Err("Invalid chart settings".into())}
 let mut out=std::collections::BTreeSet::new();
 for i in 0..levels.pow(4){let mut q=i;let mut v=[0.;4];for j in (0..4).rev(){v[j]=caps[j]*(q%levels) as f64/(levels-1) as f64;q/=levels;}
  if v.iter().sum::<f64>()<=tac+1e-9{out.insert(v.map(|x|(x/100.*65535.).floor() as u16));}
 }
 Ok(out.into_iter().collect())
}
pub fn generate_chart(reference:&Path,root:&Path,levels:usize,caps:[f64;4],tac:f64,dpi:u32)->R<PathBuf>{
 let profile=crate::comparison::open_printer(reference)?;let data=profile.icc().map_err(|e|e.to_string())?;
 let recipes=chart_recipes(levels,caps,tac)?;
 let cols=(recipes.len() as f64).sqrt().ceil() as u32;let rows=(recipes.len() as u32).div_ceil(cols);
 if !(50..=1200).contains(&dpi){return Err("Chart DPI 50–1200".into())}
 let patch=((8.*dpi as f64/25.4).round() as u32).max(4);let gap=((2.*dpi as f64/25.4).round() as u32).max(1);let pitch=patch+gap;
 let w=cols*pitch+gap;let h=rows*pitch+gap;
 if w as u64*h as u64>24_000_000{return Err("Chart exceeds 24 MP; reduce levels or DPI".into())}
 let mut pixels=vec![[0u16;4];(w*h) as usize];let mut csv=format!("{HEADER}\n");let mut layout="id,row,column,x_px,y_px,patch_px\n".to_owned();
 for (i,&v) in recipes.iter().enumerate(){let x=gap+(i as u32%cols)*pitch;let y=gap+(i as u32/cols)*pitch;
  for r in y..y+patch{for c in x..x+patch{pixels[(r*w+c) as usize]=v;}}
  let p=v.map(|n|100.*n as f64/65535.);let id=i+1;
  csv.push_str(&format!("P{id:04},{:.6},{:.6},{:.6},{:.6},,,\n",p[0],p[1],p[2],p[3]));
  layout.push_str(&format!("P{id:04},{},{},{x},{y},{patch}\n",i as u32/cols+1,i as u32%cols+1));
 }
 let stage=tempfile::tempdir_in(root).map_err(|e|e.to_string())?;
 crate::chart_tiff::write_cmyk_tiff(&stage.path().join("chart.tif"),w,h,&pixels,&data,dpi)?;
 fs::write(stage.path().join("measurements.csv"),csv).map_err(|e|e.to_string())?;
 fs::write(stage.path().join("layout.csv"),layout).map_err(|e|e.to_string())?;
 fs::write(stage.path().join("README.txt"),format!("{} patches, 8 mm patches, 2 mm gaps at {} DPI. Direct device CMYK values; RIP must preserve channel values with no reconversion. Print under documented glaze/ink/firing conditions. Measure D50 Lab consistently and fill the blank L,a,b fields. No measurements are generated. Verify physical safe ink limits before printing. CSV row/column are 1-based; x/y pixel coordinates are 0-based.",recipes.len(),dpi)).map_err(|e|e.to_string())?;
 let dest=root.join("ceramic_chart");if dest.exists(){return Err("ceramic_chart folder already exists".into())}
 fs::rename(stage.path(),&dest).map_err(|e|e.to_string())?;Ok(dest)
}
pub fn profile_from_ti3(source:&Path,destination:&Path,tac:f64)->R<()>{
 if !tac.is_finite()||!(0.0..=400.).contains(&tac){return Err("Invalid TAC".into())}
 if fs::metadata(source).map_err(|e|e.to_string())?.len()>64*1024*1024{return Err("TI3 too large".into())}
 let stage=tempfile::tempdir().map_err(|e|e.to_string())?;let base=stage.path().join("measurements");
 fs::copy(source,base.with_extension("ti3")).map_err(|e|e.to_string())?;
 let revfix=crate::backend::engine(&|_|{})?;let exe=revfix.with_file_name("colprof.exe");
 if !exe.is_file(){return Err("colprof.exe not present in official Argyll installation".into())}
 let out=stage.path().join("profile.icc");
 let result=crate::backend::hidden(&mut std::process::Command::new(exe)).current_dir(stage.path()).args(["-v","-qm"]).arg(format!("-l{tac}")).arg("-O").arg(&out).arg(&base).output().map_err(|e|e.to_string())?;
 if !result.status.success(){return Err(format!("colprof: {}\n{}",String::from_utf8_lossy(&result.stderr),String::from_utf8_lossy(&result.stdout)))}
 let _=crate::comparison::open_printer(&out)?;let bytes=fs::read(&out).map_err(|e|e.to_string())?;
 let mut t=tempfile::NamedTempFile::new_in(destination.parent().unwrap_or(Path::new("."))).map_err(|e|e.to_string())?;
 use std::io::Write;t.write_all(&bytes).map_err(|e|e.to_string())?;t.persist_noclobber(destination).map_err(|e|e.to_string())?;Ok(())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn chart_respects_limits(){let p=chart_recipes(5,[100.,100.,100.,0.],180.).unwrap();assert!(!p.is_empty());assert!(p.iter().all(|v|v[3]==0&&v.iter().map(|&v|v as f64*100./65535.).sum::<f64>()<=180.+1e-9));}
 #[test]fn measured_pair(){let d=tempfile::tempdir().unwrap();let a=d.path().join("a.csv");let b=d.path().join("b.csv");fs::write(&a,format!("{HEADER}\np1,0,0,0,0,50,2.6772,-79.7751\n")).unwrap();fs::write(&b,format!("{HEADER}\np1,0,0,0,0,50,0,-82.7485\n")).unwrap();assert!((compare(&a,&b).unwrap().mean-2.0425).abs()<0.0001);}
 #[test]fn rejects_blank(){let d=tempfile::tempdir().unwrap();let a=d.path().join("a.csv");fs::write(&a,format!("{HEADER}\np1,0,0,0,0,,,\n")).unwrap();assert!(load_csv(&a).is_err());}
}
