//! Reusable GPUI curve graph: normalized grid, identity guide, curve and handles.
//! Interaction is attached by the owning editor; no Canvas or texture dependency.
use gpui::{prelude::*,*};
pub const HEIGHT:f32=126.;
pub fn graph(samples:&[f64],color:u32)->Div{
 let mut graph=div().relative().w_full().h(px(HEIGHT)).flex_shrink_0().bg(rgb(0x101a2a)).rounded_md().overflow_hidden();
 for i in 0..=4{
  let t=i as f32/4.;
  graph=graph.child(div().absolute().left(relative(t)).top(px(0.)).w(px(1.)).h_full().bg(rgb(0x2c3b50)))
   .child(div().absolute().left(px(0.)).top(px(t*(HEIGHT-1.))).h(px(1.)).w_full().bg(rgb(0x2c3b50)));
 }
 for i in 0..=100{let t=i as f32/100.;graph=graph.child(div().absolute().left(relative(t)).top(px((1.-t)*(HEIGHT-3.))).size(px(2.)).bg(rgb(0x526175)));}
 // Dense samples form a continuous visible stroke at ordinary card widths.
 for (i,&v) in samples.iter().enumerate(){
  let t=i as f32/(samples.len()-1).max(1) as f32;
  graph=graph.child(div().absolute().left(relative(t)).top(px((1.-v.clamp(0.,1.) as f32)*(HEIGHT-3.))).size(px(3.)).rounded_md().bg(rgb(color)));
 }
 graph
}
pub fn handle(id:impl Into<ElementId>,x:f32,y:f64,color:u32)->Stateful<Div>{
 div().id(id).absolute().left(relative(x)).top(px((1.-y.clamp(0.,1.) as f32)*(HEIGHT-12.))).size(px(12.)).rounded_md().bg(rgb(color)).cursor_pointer().hover(|s|s.bg(rgb(0xffffff)))
}
pub fn linear(points:[f64;5],x:f64)->f64{let p=x.clamp(0.,1.)*4.;let i=(p.floor() as usize).min(3);points[i]+(points[i+1]-points[i])*(p-i as f64)}
