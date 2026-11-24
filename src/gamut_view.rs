//! GPUI image element wrapper. Fixed nonzero square and no Canvas paint hooks.
use gpui::{prelude::*,*};
use std::sync::Arc;
use color_convert_rust::{gamut::Overlay,gamut_raster};
pub use gamut_raster::ViewOptions;
pub fn plot(data:Option<&Overlay>,o:ViewOptions)->impl IntoElement{
 let rendered=gamut_raster::render(data,o);
 let label=format!("Visible points: {} before / {} after · ±{:.0} Lab units",rendered.visible[0],rendered.visible[1],rendered.units_per_half);
 let mut root=div().flex().flex_col().items_center().gap_1().w_full().flex_shrink_0();
 match rendered.png(){
  Ok(png)=>{
   let image=Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png,png));
   root=root.child(div().w(px(320.)).h(px(320.)).flex_shrink_0().bg(rgb(0x0b1220)).child(img(image).w(px(320.)).h(px(320.)).flex_shrink_0()));
  }
  Err(e)=>{root=root.child(div().w(px(320.)).h(px(320.)).bg(rgb(0x0b1220)).child(format!("Preview rendering failed: {e}")));}
 }
 root.child(div().text_xs().child(label))
}
