#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use color_convert_rust::Profile;
use eframe::egui;
#[cfg(windows)] mod updates;
const ICON: &[u8] = include_bytes!("../assets/icon.png");
use std::{path::PathBuf,sync::mpsc,thread};
type R<T>=Result<T,String>;
use color_convert_rust::backend::{run,Job,Mode};
enum Event{Progress(String),Done(R<String>)}
struct App{
 source:Option<PathBuf>,destination:Option<PathBuf>,profile:Option<Profile>,names:Vec<String>,caps:[f64;4],tac:f64,estimate:bool,grid:usize,mode:Mode,status:String,busy:bool,rx:Option<mpsc::Receiver<Event>>,about:bool,icon:Option<egui::TextureHandle>,#[cfg(windows)] updater:updates::Updates
}
impl Default for App{fn default()->Self{Self{source:None,destination:None,profile:None,names:vec![],caps:[100.,100.,100.,0.],tac:300.,estimate:true,grid:17,mode:Mode::Argyll,status:"Choose a reference ICC printer profile.".into(),busy:false,rx:None,about:false,icon:None,#[cfg(windows)] updater:updates::Updates::default()}}}
impl App{
 fn open(&mut self,p:PathBuf){
  self.source=Some(p.clone());self.profile=None;self.destination=None;self.names.clear();
  match Profile::read(&p){Ok(profile)=>{
   self.names=profile.channels();self.caps=[100.;4];if profile.cmyk(){self.caps[3]=0.;self.mode=if profile.version_major()==4{Mode::Custom}else{Mode::Argyll}}else{self.mode=Mode::Custom}
   match profile.check(){Ok(())=>{self.status=format!("Detected ICC v{} / {} / PCS {}. Confirm physical ink order before rebuilding.",profile.version_major(),profile.space(),profile.pcs());if profile.version_major()==4{self.grid=33;}self.tac=profile.estimated_tac_percent().unwrap_or(300.);self.profile=Some(profile);self.destination=Some(p.with_file_name(format!("{}_limited.icc",p.file_stem().unwrap_or_default().to_string_lossy())));}
   Err(e)=>self.status=e}
  }Err(e)=>self.status=e}
 }
 fn start(&mut self,ctx:&egui::Context){
  let (Some(source),Some(destination))=(self.source.clone(),self.destination.clone())else{return};
  let j=Job{source,destination,mode:self.mode,caps:self.caps,tac:self.tac,estimate:self.estimate,grid:self.grid};
  let (tx,rx)=mpsc::channel();self.rx=Some(rx);self.busy=true;self.status="Starting profile rebuild...".into();let ctx=ctx.clone();
  thread::spawn(move||{let notify=|s|{let _=tx.send(Event::Progress(s));ctx.request_repaint();};let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||run(j,notify))).unwrap_or_else(|_|Err("Unexpected engine failure".into()));let _=tx.send(Event::Done(result));ctx.request_repaint();});
 }
}
impl eframe::App for App{
 fn update(&mut self,ctx:&egui::Context,_frame:&mut eframe::Frame){
  #[cfg(windows)] self.updater.tick(ctx);
  let mut done=false;
  if let Some(rx)=&self.rx{loop{match rx.try_recv(){Ok(Event::Progress(s))=>self.status=s,Ok(Event::Done(r))=>{self.status=r.unwrap_or_else(|e|format!("Error: {e}"));done=true;break},Err(mpsc::TryRecvError::Empty)=>break,Err(mpsc::TryRecvError::Disconnected)=>{self.status="Worker disconnected.".into();done=true;break}}}}
  if done{self.rx=None;self.busy=false}
  if !self.busy{if let Some(p)=ctx.input(|i|i.raw.dropped_files.first().and_then(|f|f.path.clone())){self.open(p)}}
  egui::CentralPanel::default().show(ctx,|ui|{
   ui.horizontal(|ui|{
    if let Some(icon)=&self.icon {ui.image((icon.id(),egui::vec2(42.,42.)));}
    ui.vertical(|ui|{ui.heading("ICC Profile Rebuilder");ui.small("Portable color separation utility");});
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
      if ui.button("About / Updates").clicked(){self.about=true;}
      ui.small(format!("v{}",env!("CARGO_PKG_VERSION")));
    });
   });
   ui.separator();
   ui.add_enabled_ui(!self.busy,|ui|{
    if ui.button("Open reference ICC / ICM").clicked(){if let Some(p)=rfd::FileDialog::new().add_filter("ICC Profile",&["icc","icm"]).pick_file(){self.open(p);}}
    if let Some(p)=&self.source{ui.label(p.display().to_string());}
    if self.profile.is_some(){
     ui.separator();ui.heading("Channel limits");
     ui.label("Names are editable labels only. Physical channel positions are never reordered.");
     ui.label("CMYK labels may not match ceramic inks unless the ICC has colorant metadata.");
     egui::Grid::new("channels").num_columns(4).striped(true).show(ui,|ui|{
      ui.strong("Index");ui.strong("Ink label");ui.strong("Enabled");ui.strong("Maximum");ui.end_row();
      for i in 0..4{
       ui.label((i+1).to_string());ui.text_edit_singleline(&mut self.names[i]);let mut enabled=self.caps[i]>0.;
       if ui.checkbox(&mut enabled,"").changed(){self.caps[i]=if enabled{100.}else{0.};}
       ui.add(egui::Slider::new(&mut self.caps[i],0.0..=100.0).step_by(1.).suffix("%"));ui.end_row();
      }
     });
     ui.horizontal(|ui|{ui.label("K presets:");for v in [0.,5.,10.,100.]{if ui.button(format!("{v}%")).clicked(){self.caps[3]=v;}}});
     let can_argyll=self.profile.as_ref().map(|p|p.cmyk()&&p.version_major()==2).unwrap_or(false)&&self.caps[..3].iter().all(|&v|v==100.);
     if !can_argyll{self.mode=Mode::Custom}
     ui.separator();ui.horizontal(|ui|{
      ui.label("Engine:");
      ui.add_enabled_ui(can_argyll,|ui|{ui.radio_value(&mut self.mode,Mode::Argyll,"ArgyllCMS revfix");});
      ui.radio_value(&mut self.mode,Mode::Custom,"Custom constrained LUT");
     });
     if self.mode==Mode::Argyll{
      ui.label("Argyll is installed beside the EXE automatically on first generation.");
      ui.checkbox(&mut self.estimate,"Estimate total ink from reference ICC");
     }else{
      if self.profile.as_ref().is_some_and(|p|p.version_major()==4){ui.label("ICC v4 detected: Argyll revfix disabled; original version preserved.");}
      ui.label("Custom ICC v2 mft2 / v4 mAB (sampled curves). TAC starts from the reference B2A1 node estimate when supported. Confirm safe TAC with your print calibration.");
     }
     if self.mode==Mode::Custom||!self.estimate{ui.add(egui::Slider::new(&mut self.tac,0.0..=400.).step_by(1.).suffix("%").text("Total ink (TAC)"));}
     ui.horizontal(|ui|{ui.label("CLUT grid:");egui::ComboBox::from_id_salt("grid").selected_text(format!("{}³",self.grid)).show_ui(ui,|ui|{for v in [9,17,25,33]{ui.selectable_value(&mut self.grid,v,format!("{v}³"));}});});
     ui.separator();
     if ui.button("Choose new output ICC").clicked(){let mut dlg=rfd::FileDialog::new().add_filter("ICC Profile",&["icc","icm"]);
      if let Some(p)=&self.destination{if let Some(n)=p.file_name(){dlg=dlg.set_file_name(n.to_string_lossy().to_string());}if let Some(d)=p.parent(){dlg=dlg.set_directory(d);}}
      if let Some(mut p)=dlg.save_file(){if p.extension().is_none(){p.set_extension("icc");}self.destination=Some(p);}
     }
     if let Some(p)=&self.destination{ui.label(p.display().to_string());}
     if ui.button("Generate and verify ICC").clicked(){self.start(ctx);}
    }
   });
   ui.separator();
   if self.busy{
    if let Some((n,total))=self.status.strip_prefix("Inverting A2B1: slice ").and_then(|s|s.split_once('/')).and_then(|(a,b)|Some((a.parse::<f32>().ok()?,b.parse::<f32>().ok()?))) {
     ui.add(egui::ProgressBar::new(((n-1.)/total.max(1.)).clamp(0.,1.)).text(format!("Rebuilding slice {n:.0} of {total:.0}")));
    } else {ui.horizontal(|ui|{ui.spinner();ui.label("Working — exact progress unavailable");});}
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
   } else if self.status.starts_with("Saved:") {ui.add(egui::ProgressBar::new(1.).text("Complete — ICC verified"));}
   egui::ScrollArea::vertical().max_height(190.).show(ui,|ui|{ui.label(&self.status);});
   ui.label("Rebuilt intents use relative-colorimetric inversion. Real print color and firing must be tested.");
  });
  let mut open=self.about;
  if open {
   egui::Window::new("About ICC Profile Rebuilder").open(&mut open).collapsible(false).resizable(true).default_width(420.).show(ctx,|ui|{
    ui.vertical_centered(|ui|{
     if let Some(icon)=&self.icon {ui.image((icon.id(),egui::vec2(88.,88.)));}
     ui.heading("ICC Profile Rebuilder");
     ui.label(format!("Version {}  ·  Windows x64  ·  Portable",env!("CARGO_PKG_VERSION")));
     ui.small("Developed by emadgh");
    });
    ui.separator();
    ui.label("Rebuild printer ICC profiles with constrained ink channels while preserving the original channel layout.");
    ui.label("Processing engines: ArgyllCMS revfix and experimental constrained LUT.");
    ui.hyperlink_to("Project on GitHub","https://github.com/emadgh/color-convert-rust");
    ui.hyperlink_to("Shared updater — update-via-github","https://github.com/emadgh/update-via-github");
    ui.hyperlink_to("ArgyllCMS documentation","https://www.argyllcms.com/doc/revfix.html");
    ui.separator();
    ui.heading("Software updates");
    #[cfg(windows)] self.updater.show(ui,self.busy);
    #[cfg(not(windows))] ui.label("Updater requires Windows.");
    ui.separator();
    ui.small("Validate real print results after firing before production use.");
   });
  }
  self.about=open;
 }
}
fn main()->eframe::Result<()>{
 let img=image::load_from_memory(ICON).expect("Embedded icon must be valid").into_rgba8();
 let (w,h)=img.dimensions();
 let rgba=img.into_raw();
 let icon=egui::IconData{rgba:rgba.clone(),width:w,height:h};
 let options=eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size([780.,760.]).with_min_inner_size([620.,650.]).with_icon(icon),..Default::default()};
 eframe::run_native("ICC Profile Rebuilder",options,Box::new(move|cc|{
  let mut app=App::default();
  app.icon=Some(cc.egui_ctx.load_texture("app-icon",egui::ColorImage::from_rgba_unmultiplied([w as usize,h as usize],&rgba),egui::TextureOptions::LINEAR));
  Ok(Box::new(app))
 }))
}
