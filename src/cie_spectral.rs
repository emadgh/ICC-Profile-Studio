//! Direct 1 nm colorimetry using official CIE data, not bandpass correction.
use crate::R;
use lcms2::{CIEXYZ,CIEXYZExt};
use std::sync::OnceLock;
const START:u32=360;const END:u32=830;
const CMF2:&str=include_str!(concat!(env!("OUT_DIR"),"/CIE_xyz_1931_2deg.csv"));
const CMF10:&str=include_str!(concat!(env!("OUT_DIR"),"/CIE_xyz_1964_10deg.csv"));
const D50:&str=include_str!(concat!(env!("OUT_DIR"),"/CIE_std_illum_D50.csv"));
const D65:&str=include_str!(concat!(env!("OUT_DIR"),"/CIE_std_illum_D65.csv"));
const A:&str=include_str!(concat!(env!("OUT_DIR"),"/CIE_std_illum_A_1nm.csv"));
pub const ATTRIBUTION:&str=include_str!("../CIE_DATA.md");
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Illuminant{D50,D65,A}
impl Illuminant{pub const ALL:[Self;3]=[Self::D50,Self::D65,Self::A];pub fn label(self)->&'static str{match self{Self::D50=>"D50",Self::D65=>"D65",Self::A=>"A"}}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Observer{Two,Ten}
impl Observer{pub const ALL:[Self;2]=[Self::Two,Self::Ten];pub fn label(self)->&'static str{match self{Self::Two=>"2°",Self::Ten=>"10°"}}}
#[derive(Clone,Copy,Debug)]
pub struct ResultColor{pub xyz:[f64;3],pub white:[f64;3],pub lab:[f64;3],pub endpoint_extended:bool}
#[derive(Debug)]
struct Weights{rows:Vec<[f64;3]>,white:[f64;3]}
fn table<const N:usize>(data:&str,first:u32,allow_z_tail:bool)->R<Vec<[f64;N]>>{
 if data.is_empty(){return Err("Official CIE tables not embedded; run scripts/fetch-cie.ps1 and rebuild".into())}
 let mut out=Vec::new();
 for (line_index,line) in data.trim_start_matches('\u{feff}').lines().enumerate(){
  let line=line.trim();if line.is_empty(){continue}
  let mut fields=line.split(',');
  let raw=fields.next().ok_or("Missing wavelength")?.trim().trim_matches('"');
  if out.is_empty()&&matches!(raw.to_ascii_lowercase().as_str(),"wavelength"|"wavelength (nm)"|"wavelength [nm]"|"lambda"|"nm"){continue}
  let value=raw.parse::<f64>().map_err(|_|format!("Invalid CIE wavelength at line {}: {:?}",line_index+1,raw))?;
  if !value.is_finite()||value.fract()!=0.||!(0.0..=1000.).contains(&value){return Err("Invalid integral CIE wavelength".into())}
  let nm=value as u32;
  if nm!=first+out.len() as u32||nm>END{return Err("Unexpected CIE wavelength grid".into())}
  let mut row=[0.;N];
  for (j,slot) in row.iter_mut().enumerate(){
   let s=fields.next().ok_or("Missing CIE value")?.trim().trim_matches('"');
   *slot=if allow_z_tail&&j==2&&nm>=560&&(s.eq_ignore_ascii_case("nan")||s.is_empty()){0.}else{s.parse::<f64>().map_err(|_|"Invalid CIE value")?};
   if !slot.is_finite()||*slot<0.{return Err("Invalid CIE table value".into())}
  }
  if fields.next().is_some(){return Err("Unexpected CIE column".into())}
  out.push(row);
 }
 if out.len()!=(END-first+1) as usize{return Err("Incomplete CIE table".into())}
 Ok(out)
}
fn build()->R<Vec<Weights>>{
 let observers=[table::<3>(CMF2,START,false).map_err(|e|format!("CIE 1931: {e}"))?,table::<3>(CMF10,START,true).map_err(|e|format!("CIE 1964: {e}"))?];
 let illuminants=[table::<1>(D50,300,false).map_err(|e|format!("D50: {e}"))?,table::<1>(D65,300,false).map_err(|e|format!("D65: {e}"))?,table::<1>(A,300,false).map_err(|e|format!("A: {e}"))?];
 let mut all=Vec::new();
 for cmf in &observers{for illum in &illuminants{
  let mut rows=Vec::new();let mut white=[0.;3];
  for (i,f) in cmf.iter().enumerate(){
   let s=illum[i+(START-300) as usize][0];
   let w=f.map(|v|v*s);for j in 0..3{white[j]+=w[j];}rows.push(w);
  }
  let norm=white[1];if !norm.is_finite()||norm<=0.{return Err("Invalid reference white".into())}
  white=white.map(|v|v/norm);for row in &mut rows{for x in row{*x/=norm;}}
  all.push(Weights{rows,white});
 }}
 Ok(all)
}
static DATA:OnceLock<R<Vec<Weights>>>=OnceLock::new();
fn data()->R<&'static [Weights]>{DATA.get_or_init(build).as_ref().map(Vec::as_slice).map_err(Clone::clone)}
pub fn available()->bool{data().is_ok()}
/// 1–20 nm uniform samples, at least 380–730 nm coverage.
/// Constant extension to 360–830 nm is explicitly reported.
pub fn calculate(r:&[f64],start:u32,step:u32,ill:Illuminant,obs:Observer)->R<ResultColor>{
 if !(1..=20).contains(&step)||r.len()<2||r.len()>1024||start<START||start>380{return Err("Unsupported spectral grid".into())}
 let end=start.checked_add(step.checked_mul((r.len()-1) as u32).ok_or("Wavelength overflow")?).ok_or("Wavelength overflow")?;
 if end<730||end>END||r.iter().any(|v|!v.is_finite()||*v<0.||*v>2.){return Err("Invalid reflectance or insufficient wavelength coverage".into())}
 let i=match obs{Observer::Two=>0,Observer::Ten=>3}+match ill{Illuminant::D50=>0,Illuminant::D65=>1,Illuminant::A=>2};
 let w=&data()?[i];let mut xyz=[0.;3];
 for (i,row) in w.rows.iter().enumerate(){
  let nm=START+i as u32;
  let q=nm.saturating_sub(start) as f64/step as f64;
  let j=(q.floor() as usize).min(r.len()-1);let k=(j+1).min(r.len()-1);
  let value=if j==k{r[j]}else{r[j]+(r[k]-r[j])*(q-j as f64)};
  for c in 0..3{xyz[c]+=row[c]*value;}
 }
 let white=CIEXYZ{X:w.white[0],Y:w.white[1],Z:w.white[2]};
 let l=CIEXYZ{X:xyz[0],Y:xyz[1],Z:xyz[2]}.to_lab(&white);
 let lab=[l.L,l.a,l.b];if lab.iter().any(|v|!v.is_finite()){return Err("Invalid spectral Lab".into())}
 Ok(ResultColor{xyz,white:w.white,lab,endpoint_extended:start>START||end<END})
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn official_reference_whites_and_constant_reflectance(){
  if !available(){assert!(std::env::var_os("REQUIRE_CIE_DATA").is_none(),"{}",data().unwrap_err());return}
  for obs in Observer::ALL{for ill in Illuminant::ALL{
   let r=calculate(&[1.;36],380,10,ill,obs).unwrap();
   assert!(r.endpoint_extended);assert!((r.lab[0]-100.).abs()<1e-7&&r.lab[1].abs()<1e-7&&r.lab[2].abs()<1e-7);
   let half=calculate(&[0.5;36],380,10,ill,obs).unwrap();assert!((half.lab[0]-76.069261).abs()<0.0001);
   if obs==Observer::Two{
    let expected=match ill{Illuminant::D50=>[0.9642,1.,0.8251],Illuminant::D65=>[0.95047,1.,1.08883],Illuminant::A=>[1.0985,1.,0.35585]};
    assert!(r.white.iter().zip(expected).all(|(x,y)|(x-y).abs()<0.002));
   }
  }}
 }
 #[test]fn refuse_invalid_grid(){assert!(calculate(&[0.5;35],380,10,Illuminant::D50,Observer::Two).is_err());assert!(calculate(&[f64::NAN;36],380,10,Illuminant::D50,Observer::Two).is_err());}
 #[test]fn parser_rejects_missing_data(){assert!(table::<3>("360,1,2,3\n362,1,2,3",360,false).is_err());}
}
