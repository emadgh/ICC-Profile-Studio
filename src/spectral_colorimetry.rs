//! Legacy D50 / CIE 1931 2-degree spectral integration for 10 nm data.
//! Numeric weights: ITU-T T.42 (11/94), Appendix I, Table I.1,
//! reproduced there from ASTM E308-1985.
//! https://www.itu.int/rec/T-REC-T.42-199411-S/en
//! This is explicitly a legacy estimate, not ISO 13655:2017 certification.
//! For 380–730 nm, extend endpoint reflectance to 360–780 nm;
//! normalize using the same weight sums so a perfect diffuser maps to D50.
//! M0 spectra cannot be transformed into an actual M1 measurement.
use crate::R;
use lcms2::{CIEXYZ,CIEXYZExt};
const W:[[u32;3];43]=[
 [0,0,1],
 [1,0,5],
 [3,0,13],
 [12,0,57],
 [60,2,285],
 [234,6,1113],
 [775,23,3723],
 [1610,66,7862],
 [2453,162,12309],
 [2777,313,14647],
 [2500,514,14346],
 [1717,798,11299],
 [861,1239,7309],
 [283,1839,4128],
 [40,2948,2466],
 [88,4632,1447],
 [593,6587,736],
 [1590,8308,401],
 [2799,9197,196],
 [4207,9650,85],
 [5657,9471,37],
 [7132,8902,20],
 [8540,8112,15],
 [9255,6829,10],
 [9835,5838,7],
 [9469,4753,4],
 [8009,3573,2],
 [5926,2443,1],
 [4171,1629,0],
 [2609,984,0],
 [1541,570,0],
 [855,313,0],
 [434,158,0],
 [194,70,0],
 [97,35,0],
 [50,18,0],
 [22,8,0],
 [12,4,0],
 [6,2,0],
 [2,1,0],
 [1,0,0],
 [1,0,0],
 [0,0,0],
];
pub(crate) fn lab(values:&[f64],start:u32,step:u32)->R<[f64;3]>{
 if step!=10||!matches!((start,values.len()),(380,36)|(380,41)|(360,43)){
  return Err("Legacy D50 integration supports 380–730, 380–780 or 360–780 nm at 10 nm only".into())
 }
 if values.iter().any(|v|!v.is_finite()||*v<0.||*v>2.){return Err("Invalid normalized reflectance".into())}
 let mut sum=[0.;3];let mut white=[0.;3];
 for (i,w) in W.iter().enumerate(){
  let nm=360+10*i as u32;
  let j=(nm.saturating_sub(start)/10) as usize;
  let r=values[j.min(values.len()-1)];
  for k in 0..3{sum[k]+=r*f64::from(w[k]);white[k]+=f64::from(w[k]);}
 }
 let d50=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 let l=CIEXYZ{X:d50.X*sum[0]/white[0],Y:sum[1]/white[1],Z:d50.Z*sum[2]/white[2]}.to_lab(&d50);
 if [l.L,l.a,l.b].iter().any(|v|!v.is_finite()){return Err("Invalid spectral Lab".into())}
 Ok([l.L,l.a,l.b])
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn checksums_and_neutral_reflectance(){
  let sums=[0,1,2].map(|j|W.iter().map(|v|v[j]).sum::<u32>());assert_eq!(sums,[96421,99997,82524]);
  let l=lab(&[1.;36],380,10).unwrap();assert!((l[0]-100.).abs()<1e-9&&l[1].abs()<1e-9&&l[2].abs()<1e-9);
  let l=lab(&[0.5;36],380,10).unwrap();assert!((l[0]-76.069261).abs()<0.0001&&l[1].abs()<1e-9&&l[2].abs()<1e-9);
  assert!(lab(&[0.5;35],380,10).is_err());
 }
}
