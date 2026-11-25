//! Pure UI scale and plotting primitives; no ICC math changes.
#[derive(Clone,Copy,Debug)]
pub struct Scale{pub low:f64,pub high:f64,pub step:f64}
impl Scale{
 pub fn fraction(self,value:f64)->f64{
  if !value.is_finite()||!self.low.is_finite()||!self.high.is_finite()||self.high<=self.low{return 0.}
  ((value-self.low)/(self.high-self.low)).clamp(0.,1.)
 }
 pub fn at(self,fraction:f64)->f64{
  if !fraction.is_finite()||self.high<=self.low||!self.step.is_finite()||self.step<=0.{return self.low}
  if fraction<=0.{return self.low}if fraction>=1.{return self.high}
  (self.low+((self.high-self.low)*fraction/self.step).round()*self.step).clamp(self.low,self.high)
 }
 pub fn adjust(self,value:f64,direction:i32)->f64{
  if !value.is_finite()||!self.step.is_finite()||self.step<=0.||self.high<=self.low{return self.low}
  (value+direction as f64*self.step).clamp(self.low,self.high)
 }
 pub fn parse(self,text:&str)->Option<f64>{
  if !self.low.is_finite()||!self.high.is_finite()||self.high<self.low||!self.step.is_finite()||self.step<=0.{return None}
  let normalized=normalize_numeric(text)?;
  let value=normalized.parse::<f64>().ok()?;if !value.is_finite()||value<self.low||value>self.high{return None}
  if self.high==self.low{return Some(self.low)}
  Some(self.at(self.fraction(value)))
 }
 pub fn label(self,value:f64)->String{
  let decimals=if self.step>=1.{0}else if self.step>=0.1{1}else if self.step>=0.01{2}else{3};
  format!("{value:.decimals$}")
 }
}
/// Shared whole-field grammar for typing/pasting a numeric value. No grouping,
/// exponents, units or multiline input; never reinterpret an ambiguous value.
pub fn normalize_numeric(text:&str)->Option<String>{
 if text.len()>96{return None}
  let normalized:String=text.trim().chars().map(|c|match c{'۰'..='۹'=>char::from_u32('0' as u32+c as u32-'۰' as u32).unwrap(),'٠'..='٩'=>char::from_u32('0' as u32+c as u32-'٠' as u32).unwrap(),'٫'|','=>'.','−'=>'-',_=>c}).collect();
  if normalized.is_empty()||normalized.len()>24||!normalized.chars().all(|c|c.is_ascii_digit()||matches!(c,'.'|'+'|'-')){return None}

 Some(normalized)
}
/// Map samples to equally spaced x coordinates. Missing/nonfinite values
/// break the line rather than implying zero or bridging missing data.
pub fn segments(values:&[f64],maximum:f64)->Vec<[[f32;2];2]>{
 if values.len()<2||!maximum.is_finite()||maximum<=0.{return vec![]}
 let d=(values.len()-1) as f32;
 values.windows(2).enumerate().filter_map(|(i,p)|{
  if !p.iter().all(|v|v.is_finite()){return None}
  Some([[i as f32/d,(p[0]/maximum).clamp(0.,1.) as f32],[(i+1) as f32/d,(p[1]/maximum).clamp(0.,1.) as f32]])
 }).collect()
}
pub fn line_steps(a:[f32;2],b:[f32;2],width:f32,height:f32)->usize{
 (((b[0]-a[0]).abs()*width).max((b[1]-a[1]).abs()*height).ceil() as usize).clamp(1,4096)
}
/// Compact windows use the expanded viewer rather than stealing editor space.
pub fn docked_preview(requested:bool,width:f32)->bool{requested&&width>=1100.}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn scale_endpoints_and_degenerate_range(){let s=Scale{low:0.,high:1.,step:0.3};assert_eq!(s.at(1.),1.);assert_eq!(s.at(0.),0.);assert!((s.at(0.5)-0.6).abs()<1e-12);assert_eq!(Scale{low:1.,high:1.,step:0.1}.fraction(1.),0.);assert_eq!(s.adjust(0.9,1),1.);}
 #[test]fn localized_numeric_input(){let s=Scale{low:-10.,high:10.,step:0.1};for input in ["-1.25","−۱٫۲۵","-١,٢٥"]{assert!((s.parse(input).unwrap()+1.2).abs()<0.11);}for input in ["NaN","inf","11","1,2,3","","1e0","1 2","1%"]{assert!(s.parse(input).is_none());}assert_eq!(s.parse("10"),Some(10.));}
 #[test]fn clipboard_normalization(){assert_eq!(normalize_numeric("  −۱۲٫۵  ").as_deref(),Some("-12.5"));assert_eq!(normalize_numeric("١٢,٥").as_deref(),Some("12.5"));for text in ["1\n2","12%","1e2","1 2","\u{202e}12","1234567890123456789012345"]{assert!(normalize_numeric(text).is_none());}}
 #[test]fn plot_flat_line_and_gaps(){let s=segments(&[0.5,0.5,f64::NAN,1.,0.],1.);assert_eq!(s.len(),2);assert_eq!(line_steps(s[0][0],s[0][1],400.,100.),100);assert!(segments(&[1.,2.],0.).is_empty());}
 #[test]fn compact_view(){assert!(!docked_preview(true,1000.));assert!(docked_preview(true,1100.));assert!(!docked_preview(false,1400.));}
}
