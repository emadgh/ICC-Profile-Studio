//! Display-only workaround for the current GPUI LTR text path.
//! Shape logical Arabic/Persian first, then apply UAX #9 visual ordering.
//! Do not pass output to storage, ICC metadata, file paths, or editable fields.
use unicode_bidi::{BidiInfo,Level};
fn mirror(c:char)->char{match c{'('=>')',')'=>'(','['=>']',']'=>'[','{'=>'}','}'=>'{','<'=>'>','>'=>'<','«'=>'»','»'=>'«',_=>c}}
pub fn visual(text:&str)->String{
 if !text.chars().any(|c|matches!(c as u32,0x0600..=0x06ff|0x0750..=0x077f|0x08a0..=0x08ff)){return text.to_owned()}
 // Process explicit lines independently. This is deliberately not a
 // full text layout engine: soft wrapping is still performed by GPUI.
 text.split('\n').map(|line|{
  if !line.chars().any(|c|matches!(c as u32,0x0600..=0x06ff|0x0750..=0x077f|0x08a0..=0x08ff)){return line.to_owned()}
  let shaped=arabic_reshaper::arabic_reshape(line);
  let info=BidiInfo::new(&shaped,Some(Level::rtl()));
  let mut out=String::new();
  for p in &info.paragraphs{
   let (levels,runs)=info.visual_runs(p,p.range.clone());
   for run in runs{
    let slice=&shaped[run.clone()];
    if levels[run.start].is_rtl(){out.extend(slice.chars().rev().map(mirror));}
    else{out.push_str(slice);}
   }
  }
  out
 }).collect::<Vec<_>>().join("\n")
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn shapes_persian_and_keeps_latin_numbers(){
  let x=visual("تنظیمات ICC 120%");
  assert!(x.contains("ICC 120%")||x.contains("ICC")&&x.contains("120"));
  assert!(x.chars().any(|c|matches!(c as u32,0xfb50..=0xfdff|0xfe70..=0xfeff)));
  assert_eq!(visual("C:\\ICC\\abc.icc"),"C:\\ICC\\abc.icc");
  assert_eq!(visual(&x),x);
 }
}
