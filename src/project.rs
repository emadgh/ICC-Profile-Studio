//! Explicit, versioned workspace snapshots. No automatic job resumption.
//! ICC files stay external; SHA-256 binds stored edits to exact file bytes.
use crate::{Profile,R,correction::{Selective,BlackGeneration,MAX_REGIONS,IDENTITY_POINTS},gamut_raster::ViewOptions};
use serde::{Serialize,Deserialize};
use sha2::{Sha256,Digest};
use std::{fs,io::{Read,Write},path::{Path,PathBuf}};
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity{pub path:PathBuf,pub bytes:u64,pub sha256:String}
impl FileIdentity{
 pub fn capture(path:&Path)->R<Self>{
  let path=fs::canonicalize(path).map_err(|e|e.to_string())?;
  let mut file=fs::File::open(&path).map_err(|e|e.to_string())?;
  let bytes=file.metadata().map_err(|e|e.to_string())?.len();
  if bytes<132||bytes>128*1024*1024{return Err("Invalid ICC file size".into())}
  let mut hash=Sha256::new();let mut buf=[0u8;65536];let mut total=0u64;
  loop{let n=file.read(&mut buf).map_err(|e|e.to_string())?;if n==0{break}total+=n as u64;if total>bytes{return Err("ICC changed during hashing".into())}hash.update(&buf[..n]);}
  if total!=bytes{return Err("ICC changed during hashing".into())}
  Ok(Self{path,bytes,sha256:format!("{:x}",hash.finalize())})
 }
 pub fn verify(&self)->R<()>{
  if !self.path.is_absolute(){return Err("Project ICC paths must be absolute".into())}
  let actual=Self::capture(&self.path)?;
  if self.bytes!=actual.bytes||self.sha256!=actual.sha256{return Err(format!("ICC content changed: {}. Project not restored.",self.path.display()))}
  Ok(())
 }
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings{
 pub caps:[f64;4],pub tac:f64,pub grid:usize,pub argyll:bool,
 pub gamma:[f64;4],pub order:[usize;4],pub curve_points:[[f64;5];4],
 pub regions:[Selective;MAX_REGIONS],pub region_count:usize,pub region_selected:usize,
 pub correction_grid:usize,pub correction_tac:f64,pub smoothing:f64,pub smoothing_passes:usize,
 pub black:BlackGeneration,pub recipe_target:[f64;3],pub recipe_weights:[f64;4],pub recipe_tolerance:f64,
 pub gamut:ViewOptions,pub gamut_grid:usize,pub compare_grid:usize,pub compare_intent:u8,pub coverage_threshold:f64,
 pub preview_open:bool,pub preview_width:f32,pub chart_dpi:u32,pub lab_levels:usize,
}
impl Default for Settings{
 fn default()->Self{Self{caps:[100.;4],tac:240.,grid:33,argyll:false,gamma:[1.;4],order:[0,1,2,3],curve_points:IDENTITY_POINTS,regions:[Selective::default();MAX_REGIONS],region_count:1,region_selected:0,correction_grid:33,correction_tac:400.,smoothing:0.2,smoothing_passes:1,black:BlackGeneration::default(),recipe_target:[50.,0.,0.],recipe_weights:[1.;4],recipe_tolerance:2.,gamut:ViewOptions::default(),gamut_grid:17,compare_grid:17,compare_intent:1,coverage_threshold:2.,preview_open:true,preview_width:360.,chart_dpi:300,lab_levels:5}}
}
impl Settings{
 pub fn validate(&self)->R<()>{
  let range=|v:f64,a:f64,b:f64|v.is_finite()&&(a..=b).contains(&v);
  let mut order=self.order;order.sort_unstable();
  if self.caps.iter().any(|&x|!range(x,0.,100.))||!range(self.tac,0.,400.)||![9,17,25,33].contains(&self.grid)||self.gamma.iter().any(|&x|!range(x,0.25,4.))||order!=[0,1,2,3]||!(1..=MAX_REGIONS).contains(&self.region_count)||self.region_selected>=self.region_count||![17,33,49,65].contains(&self.correction_grid)||!range(self.correction_tac,0.,400.)||!range(self.smoothing,0.,1.)||!(1..=10).contains(&self.smoothing_passes){return Err("Invalid project edit settings".into())}
  crate::correction::validate_points(self.curve_points)?;for r in self.regions{r.check()?;}self.black.check()?;
  crate::optimizer::Settings{caps:self.caps,tac:self.tac,weights:self.recipe_weights,tolerance:self.recipe_tolerance}.validate()?;
  if !range(self.recipe_target[0],0.,100.)||self.recipe_target[1..].iter().any(|&v|!range(v,-128.,127.)){return Err("Invalid Lab recipe target".into())}
  let g=self.gamut;
  if !range(g.light as f64,0.,100.)||!g.yaw.is_finite()||!range(g.pitch as f64,-1.55,1.55)||!range(g.zoom as f64,0.5,3.)||g.opacity_2d>100||![9,17,33].contains(&self.gamut_grid)||![9,17,33].contains(&self.compare_grid)||self.compare_intent>3||!range(self.coverage_threshold,0.,20.)||!range(self.preview_width as f64,280.,4000.)||!(50..=1200).contains(&self.chart_dpi)||![3,5,7].contains(&self.lab_levels){return Err("Invalid project view settings".into())}
  Ok(())
 }
}
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project{pub schema:u32,pub reference:FileIdentity,pub candidate:Option<FileIdentity>,pub settings:Settings}
impl Project{
 pub fn validate(&self)->R<()>{
  if self.schema!=1{return Err("Unsupported project schema".into())}
  self.settings.validate()?;
  self.reference.verify()?;Profile::read(&self.reference.path)?;
  if let Some(c)=&self.candidate{c.verify()?;Profile::read(&c.path)?;}
  Ok(())
 }
}
pub fn load(path:&Path)->R<Project>{
 if fs::metadata(path).map_err(|e|e.to_string())?.len()>256*1024{return Err("Project exceeds 256 KiB".into())}
 let p:Project=serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 p.validate()?;Ok(p)
}
pub fn save(path:&Path,p:&Project,replace_checkpoint:bool)->R<()>{
 p.validate()?;
 // Only the fixed app-owned recovery path may be replaced.
 if replace_checkpoint&&path!=checkpoint()?.as_path(){return Err("Invalid checkpoint destination".into())}
 if !replace_checkpoint&&!path.extension().is_some_and(|e|e.eq_ignore_ascii_case("iccproj")){return Err("Use the .iccproj extension".into())}
 let parent=path.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new("."));
 if replace_checkpoint{fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
 let mut f=tempfile::NamedTempFile::new_in(parent).map_err(|e|e.to_string())?;
 f.write_all(&serde_json::to_vec_pretty(p).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 f.as_file().sync_all().map_err(|e|e.to_string())?;
 if replace_checkpoint{f.persist(path).map_err(|e|e.to_string())?;}else{f.persist_noclobber(path).map_err(|e|e.to_string())?;}
 Ok(())
}
pub fn checkpoint()->R<PathBuf>{Ok(crate::preferences::location()?.with_file_name("session.iccproj"))}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn roundtrip_and_hash_mismatch(){
  let d=tempfile::tempdir().unwrap();let a=d.path().join("a.icc");let b=d.path().join("b.icc");let path=d.path().join("work.iccproj");
  crate::synthetic_cmyk_v4(true).save_new(&a).unwrap();crate::synthetic_cmyk_v4(false).save_new(&b).unwrap();
  let mut p=Project{schema:1,reference:FileIdentity::capture(&a).unwrap(),candidate:Some(FileIdentity::capture(&b).unwrap()),settings:Settings::default()};
  p.settings.caps[3]=0.;p.settings.gamma[0]=1.5;p.settings.gamut.zoom=2.;save(&path,&p,false).unwrap();
  let out=load(&path).unwrap();assert_eq!(out.settings.caps[3],0.);assert_eq!(out.settings.gamma[0],1.5);assert_eq!(out.settings.gamut.zoom,2.);
  assert!(save(&path,&p,false).is_err());fs::copy(&b,&a).unwrap();assert!(load(&path).is_err());
 }
 #[test]fn rejects_invalid_settings_and_missing_files(){
  let mut s=Settings::default();s.region_count=0;assert!(s.validate().is_err());s=Settings::default();s.order=[0;4];assert!(s.validate().is_err());s=Settings::default();s.gamma[0]=f64::NAN;assert!(s.validate().is_err());
  let d=tempfile::tempdir().unwrap();let a=d.path().join("a.icc");crate::synthetic_cmyk_v4(true).save_new(&a).unwrap();let p=Project{schema:1,reference:FileIdentity::capture(&a).unwrap(),candidate:None,settings:Settings::default()};fs::remove_file(a).unwrap();assert!(p.validate().is_err());
 }
}
