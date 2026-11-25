//! Characterization filtering, paired absolute Lab point clouds and exports.
//! No inferred physical ink identity or reconstructed patch geometry.
use crate::{R,Profile,characterization::{Chart,Patch},gamut::{Cloud,Overlay}};
use std::{path::Path,io::Write,collections::HashSet};
#[derive(Clone,Copy,Debug)]
pub struct Filter{pub channel:Option<usize>,pub minimum:f64,pub maximum:f64,pub tac_max:f64,pub delta_min:Option<f64>}
impl Default for Filter{fn default()->Self{Self{channel:None,minimum:0.,maximum:100.,tac_max:1500.,delta_min:None}}}
impl Filter{
 pub fn matches(&self,p:&Patch)->bool{
  let channel=self.channel.is_none_or(|j|p.inks.get(j).is_some_and(|v|*v>=self.minimum&&*v<=self.maximum));
  channel&&p.inks.iter().sum::<f64>()<=self.tac_max+1e-9&&self.delta_min.is_none_or(|d|p.delta_e.is_some_and(|v|v>=d))
 }
}
pub fn error_color(d:Option<f64>)->u32{
 match d{Some(v) if v.is_finite()&&v>=0.&&v<=1.=>0x278c69,Some(v) if v.is_finite()&&v>=0.&&v<=2.=>0x84a94f,Some(v) if v.is_finite()&&v>=0.&&v<=5.=>0xe2aa39,Some(v) if v.is_finite()&&v>=0.=>0xd45545,_=>0x555555}
}
/// Compare precisely the same recipes, on a common absolute D50 reference.
/// This is a point cloud of the target, NOT an exhaustive gamut boundary.
pub fn paired_cloud(c:&Chart,f:Filter)->Overlay{
 let mut measured=Vec::new();let mut predicted=Vec::new();
 for p in c.patches.iter().filter(|p|f.matches(p)){
  if let (Some(a),Some(b))=(p.comparison_lab,p.absolute_lab){
   if a.iter().chain(&b).all(|x|x.is_finite()&&x.abs()<=1000.){
    measured.push(a.map(|x|x as f32));predicted.push(b.map(|x|x as f32));
   }
  }
 }
 let cloud=|points:Vec<[f32;3]>|{let sampled=points.len();if sampled==0{Err("No paired absolute Lab points".into())}else{Ok(Cloud{points,sampled,errors:vec![],surface:None})}};
 Overlay{reference:cloud(measured),candidate:Some(cloud(predicted))}
}
fn safe_id(s:&str)->bool{!s.is_empty()&&!s.chars().any(|c|c=='"'||c=='\\'||c.is_control())}
/// Export a single measured table. Never substitute predicted or estimated
/// Lab for original measured Lab. Spectra retain their normalized scale.
pub fn export_cgats(c:&Chart,path:&Path,cmyk:bool)->R<()>{
 let n=c.patches.first().ok_or("Empty chart")?.inks.len();
 if !(2..=15).contains(&n)||cmyk&&n!=4{return Err("Invalid channel count".into())}
 let mut ids=HashSet::new();
 if c.patches.iter().any(|p|p.inks.len()!=n||p.inks.iter().any(|v|!v.is_finite()||!(0.0..=100.).contains(v))||!safe_id(&p.id)||!ids.insert(&p.id)){return Err("Invalid or duplicate CGATS sample ID or ink data".into())}
 let mode=&c.patches[0].measurement;
 let consistent_mode=c.patches.iter().all(|p|&p.measurement==mode)&&safe_id(mode);
 let lab=c.patches.iter().all(|p|p.measurement_basis=="Declared D50/2°"&&p.measured_lab.is_some_and(|v|v.iter().all(|x|x.is_finite())));
 let first=&c.patches[0];
 let grid=first.start_nm.zip(first.step_nm).zip(first.spectrum.as_ref().map(Vec::len));
 let spec=grid.filter(|&((start,step),count)|step>0&&(2..=1024).contains(&count)&&start.checked_add(step.saturating_mul(count as u32-1)).is_some_and(|v|v<=2500)&&c.patches.iter().all(|p|p.start_nm==Some(start)&&p.step_nm==Some(step)&&p.spectrum.as_ref().is_some_and(|s|s.len()==count&&s.iter().all(|x|x.is_finite()&&*x>=0.))));
 if !lab&&spec.is_none(){return Err("No consistent original measured Lab or spectral table; use CSV for mixed or predicted data".into())}
 if !consistent_mode{return Err("Mixed or unspecified measurement mode; export CSV instead".into())}
 let fields=if cmyk{vec!["CMYK_C".into(),"CMYK_M".into(),"CMYK_Y".into(),"CMYK_K".into()]}else{(1..=n).map(|i|format!("CH{i}")).collect()};
 let mut header=vec!["SAMPLE_ID".to_owned()];header.extend(fields);
 if lab{header.extend(["LAB_L","LAB_A","LAB_B"].map(str::to_owned));}
 if let Some(((start,step),count))=spec{header.extend((0..count).map(|i|format!("SPEC_{}",start+i as u32*step)));}
 let mut file=tempfile::NamedTempFile::new_in(path.parent().filter(|v|!v.as_os_str().is_empty()).unwrap_or(Path::new("."))).map_err(|e|e.to_string())?;
 let mut write=||->std::io::Result<()>{
  writeln!(file,"CGATS.17\nDESCRIPTOR \"ICC Profile Studio measured characterization\"\nMEASUREMENT_CONDITION \"{mode}\"")?;
  if lab{writeln!(file,"ILLUMINANT D50\nOBSERVER 2")?;}
  if let Some(((start,step),count))=spec{writeln!(file,"SPECTRAL_NORM 1\nSPECTRAL_BANDS {count}\nSPECTRAL_START_NM {start}\nSPECTRAL_END_NM {}",start+(count as u32-1)*step)?;}
  writeln!(file,"NUMBER_OF_FIELDS {}\nBEGIN_DATA_FORMAT\n{}\nEND_DATA_FORMAT\nNUMBER_OF_SETS {}\nBEGIN_DATA",header.len(),header.join(" "),c.patches.len())?;
  for p in &c.patches{write!(file,"\"{}\"",p.id)?;for x in &p.inks{write!(file," {x:.8}")?;}
   if lab{for x in p.measured_lab.unwrap(){write!(file," {x:.8}")?;}}
   if spec.is_some(){for x in p.spectrum.as_ref().unwrap(){write!(file," {x:.10}")?;}}
   writeln!(file)?;
  }writeln!(file,"END_DATA")
 };write().map_err(|e|e.to_string())?;
 file.persist_noclobber(path).map_err(|e|e.to_string())?;Ok(())
}
/// Export an untagged CMYK16 recipe chart in newly chosen geometry.
/// Not a measured scan, not the original physical patch size. Reject recipes
/// outside explicit operator-supplied limits; never clip/change recipes.
pub fn export_page_tiff(c:&Chart,p:&Profile,page:usize,path:&Path,dpi:u32,caps:[f64;4],tac:f64)->R<()>{
 if !p.cmyk()||!(50..=1200).contains(&dpi)||caps.iter().any(|x|!x.is_finite()||!(0.0..=100.).contains(x))||!tac.is_finite()||!(0.0..=400.).contains(&tac){return Err("CMYK TIFF requires valid explicit ink limits and DPI".into())}
 let rows=c.patches.iter().filter(|p|p.page==page).collect::<Vec<_>>();
 if rows.is_empty(){return Err("Empty chart page".into())}
 let min_r=rows.iter().map(|p|p.row).min().unwrap();let min_c=rows.iter().map(|p|p.column).min().unwrap();
 let nr=rows.iter().map(|p|p.row).max().unwrap()-min_r+1;let nc=rows.iter().map(|p|p.column).max().unwrap()-min_c+1;
 if nr>100||nc>100{return Err("Page grid exceeds TIFF limit".into())}
 let side=(8.*dpi as f64/25.4).round().max(1.) as usize;let gap=(2.*dpi as f64/25.4).round().max(1.) as usize;
 let w=nc*(side+gap)+gap;let h=nr*(side+gap)+gap;
 if w*h>24_000_000{return Err("Chart exceeds 24 megapixels; lower DPI".into())}
 let mut seen=HashSet::new();let mut pixels=vec![[0u16;4];w*h];
 for patch in rows{
  if patch.inks.len()!=4||patch.inks.iter().zip(caps).any(|(&v,cap)|!v.is_finite()||v<0.||v>cap+1e-9)||patch.inks.iter().sum::<f64>()>tac+1e-9{return Err(format!("Patch {} exceeds configured ink limits; no clipping applied",patch.id))}
  if !seen.insert((patch.row,patch.column)){return Err("Duplicate patch position".into())}
  let v=std::array::from_fn(|j|((patch.inks[j]/100.*65535.).floor()) as u16);
  let x=gap+(patch.column-min_c)*(side+gap);let y=gap+(patch.row-min_r)*(side+gap);
  for yy in y..y+side{pixels[yy*w+x..yy*w+x+side].fill(v);}
 }
 let f=tempfile::NamedTempFile::new_in(path.parent().filter(|v|!v.as_os_str().is_empty()).unwrap_or(Path::new("."))).map_err(|e|e.to_string())?;
 crate::chart_tiff::write_cmyk_tiff(f.path(),w as u32,h as u32,&pixels,&[],dpi)?;
 f.persist_noclobber(path).map_err(|e|e.to_string())?;Ok(())
}
#[cfg(test)]mod tests{
 use super::*;
 fn chart()->Chart{Chart{source:"fixture".into(),original_layout:false,patches:vec![Patch{id:"p 1".into(),page:1,row:0,column:0,inks:vec![25.,50.,0.,0.],spectrum:Some(vec![0.5;36]),start_nm:Some(380),step_nm:Some(10),measurement:"M0".into(),predicted_lab:[90.,0.,0.],measured_lab:None,absolute_lab:Some([75.,0.,0.]),comparison_lab:Some([76.,0.,0.]),delta_e:Some(1.5),measurement_basis:"estimate".into()}]}}
 #[test]fn filter_and_paired_cloud(){let c=chart();let mut f=Filter::default();assert!(f.matches(&c.patches[0]));f.channel=Some(1);f.minimum=51.;assert!(!f.matches(&c.patches[0]));let o=paired_cloud(&c,Filter::default());assert_eq!(o.reference.unwrap().points,vec![[76.,0.,0.]]);assert_eq!(o.candidate.unwrap().unwrap().points,vec![[75.,0.,0.]]);}
 #[test]fn spectra_export_without_fake_lab(){let d=tempfile::tempdir().unwrap();let p=d.path().join("data.cgats");export_cgats(&chart(),&p,true).unwrap();let s=std::fs::read_to_string(&p).unwrap();assert!(!s.contains("LAB_L"));let c=crate::characterization_cgats::read(&s,4,true).unwrap();assert_eq!(c.patches[0].spectrum,Some(vec![0.5;36]));assert!(export_cgats(&chart(),&p,true).is_err());}
 #[test]fn tiff_refuses_unsafe_ink_and_preserves_recipe(){let d=tempfile::tempdir().unwrap();let path=d.path().join("chart.tif");let p=crate::synthetic_cmyk_v4(true);assert!(export_page_tiff(&chart(),&p,1,&path,50,[100.,40.,100.,0.],100.).is_err());assert!(!path.exists());export_page_tiff(&chart(),&p,1,&path,50,[100.,100.,100.,0.],100.).unwrap();let mut dec=tiff::decoder::Decoder::new(std::fs::File::open(&path).unwrap()).unwrap();assert_eq!(dec.colortype().unwrap(),tiff::ColorType::CMYK(16));let tiff::decoder::DecodingResult::U16(v)=dec.read_image().unwrap()else{panic!("expected 16-bit")};assert!(v.chunks_exact(4).any(|p|p==[16383,32767,0,0]));}
}
