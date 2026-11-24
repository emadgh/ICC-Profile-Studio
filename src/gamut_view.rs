//! GPUI image element wrapper. Fixed nonzero square and no Canvas paint hooks.
use gpui::{prelude::*,*};
use std::sync::Arc;
use color_convert_rust::{gamut::Overlay,gamut_raster};
pub use gamut_raster::ViewOptions;
#[derive(Default)]
pub struct PreviewCache{key:Option<(u64,bool,ViewOptions)>,image:Option<Arc<gpui::Image>>,label:String,error:Option<String>}
impl PreviewCache{
 fn update(&mut self,data:Option<&Overlay>,revision:u64,o:ViewOptions){
  let key=(revision,data.is_some(),o);
  if self.key==Some(key){return}
  let r=gamut_raster::render(data,o);
  self.label=format!("Visible points: {} before / {} after · ±{:.0} Lab units",r.visible[0],r.visible[1],r.units_per_half);
  self.image=None;self.error=None;
  match r.png(){Ok(png)=>self.image=Some(Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png,png))),Err(e)=>self.error=Some(e)}
  self.key=Some(key);
 }
}
pub fn plot(data:Option<&Overlay>,revision:u64,o:ViewOptions,cache:&mut PreviewCache)->impl IntoElement{
 cache.update(data,revision,o);
 let mut root=div().flex().flex_col().items_center().gap_1().w_full().flex_shrink_0();
 match cache.image.clone(){
  Some(image)=>{
   root=root.child(div().w(px(320.)).h(px(320.)).flex_shrink_0().bg(rgb(0x0b1220)).child(img(image).w(px(320.)).h(px(320.)).flex_shrink_0()));
  }
  None=>{root=root.child(div().w(px(320.)).h(px(320.)).bg(rgb(0x0b1220)).child(format!("Preview rendering failed: {}",cache.error.as_deref().unwrap_or("Unknown image error"))));}
 }
 root.child(div().text_xs().child(cache.label.clone()))
}
