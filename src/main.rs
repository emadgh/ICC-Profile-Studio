#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use color_convert_rust::Profile;
use eframe::egui;
#[cfg(windows)] mod updates;
const ICON: &[u8] = include_bytes!("../assets/icon.png");
use std::{fs,path::{Path,PathBuf},process::Command,sync::mpsc,thread};
#[cfg(windows)]use std::os::windows::process::CommandExt;
type R<T>=Result<T,String>;
const URL:&str="https://www.argyllcms.com/Argyll_V3.5.0_win64_exe.zip";
fn hidden(c:&mut Command)->&mut Command{
 #[cfg(windows)]{c.creation_flags(0x08000000);}
 c
}
fn find_exe(p:&Path,depth:u8)->Option<PathBuf>{
 if depth==0{return None}
 for e in fs::read_dir(p).ok()?.flatten(){let path=e.path();let ty=e.file_type().ok()?;if ty.is_file()&&e.file_name().to_string_lossy().eq_ignore_ascii_case("revfix.exe"){return Some(path)}if ty.is_dir(){if let Some(x)=find_exe(&path,depth-1){return Some(x)}}}None
}
fn engine(progress:&impl Fn(String))->R<PathBuf>{
 if !cfg!(all(windows,target_arch="x86_64")){return Err("Automatic Argyll installation requires Windows x64".into())}
 let exe=std::env::current_exe().map_err(|e|e.to_string())?;let parent=exe.parent().ok_or("Executable directory unavailable")?;
 let dest=parent.join("argyll-3.5.0");
 if let Some(p)=find_exe(&dest,5){return Ok(p)}
 if dest.exists(){return Err("Incomplete argyll-3.5.0 folder; rename or remove it, then retry".into())}
 progress("Downloading official ArgyllCMS 3.5.0 for Windows x64...".into());
 let temp=tempfile::tempdir_in(parent).map_err(|e|format!("Portable directory must be writable: {e}"))?;
 let zip=temp.path().join("argyll.zip");let stage=temp.path().join("unpacked");
 let ps=PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(||"C:\\Windows".into())).join("System32/WindowsPowerShell/v1.0/powershell.exe");
 let script=r#"$ErrorActionPreference='Stop'; [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -UseBasicParsing -Uri $env:ICC_ARGYLL_URL -OutFile $env:ICC_ARGYLL_ZIP; if ($env:ARGYLL_SHA256) { if ((Get-FileHash -Algorithm SHA256 -LiteralPath $env:ICC_ARGYLL_ZIP).Hash -ne $env:ARGYLL_SHA256) { throw 'Argyll archive SHA256 mismatch' } }; Expand-Archive -LiteralPath $env:ICC_ARGYLL_ZIP -DestinationPath $env:ICC_ARGYLL_STAGE"#;
 let result=hidden(&mut Command::new(ps)).args(["-NoProfile","-NonInteractive","-Command",script]).env("ICC_ARGYLL_URL",URL).env("ICC_ARGYLL_ZIP",&zip).env("ICC_ARGYLL_STAGE",&stage).output().map_err(|e|e.to_string())?;
 if !result.status.success(){return Err(format!("Dependency download failed: {}",String::from_utf8_lossy(&result.stderr)))}
 if find_exe(&stage,5).is_none(){return Err("Official archive did not contain revfix.exe".into())}
 fs::rename(&stage,&dest).map_err(|e|e.to_string())?;
 find_exe(&dest,5).ok_or("Missing revfix.exe after extraction".into())
}
#[derive(Clone,Copy,PartialEq,Eq)]
enum Mode{Argyll,Custom}
struct Job{source:PathBuf,destination:PathBuf,mode:Mode,caps:[f64;4],tac:f64,estimate:bool,grid:usize}
fn run(j:Job,progress:impl Fn(String))->R<String>{
 let reference=Profile::read(&j.source)?;reference.check()?;
 if j.destination.exists(){return Err("Output already exists. Choose a new filename; the reference is never overwritten.".into())}
 let caps=j.caps.map(|v|v/100.);
 if j.mode==Mode::Argyll && (reference.version_major()!=2||!reference.cmyk()||caps[..3].iter().any(|v|*v!=1.)){return Err("Argyll revfix requires ICC v2 CMYK and K-only limits. Use Custom for ICC v4 or other channels.".into())}
 let mut output=match j.mode{
  Mode::Custom=>reference.custom_rebuild(caps,j.tac/100.,j.grid,&progress)?,
  Mode::Argyll=>{
   let exe=engine(&progress)?;
   let dir=tempfile::tempdir().map_err(|e|e.to_string())?;
   let input=dir.path().join("input.icc");let dest=dir.path().join("output.icc");
   fs::copy(&j.source,&input).map_err(|e|e.to_string())?;
   progress("Argyll revfix is rebuilding all three B2A tables...".into());
   let mut cmd=Command::new(&exe);
   hidden(&mut cmd).current_dir(exe.parent().unwrap()).args(["-v","-0","-1","-2",if caps[3]==0.{"-kz"}else{"-kr"}]).arg(format!("-L{}",j.caps[3])).arg(format!("-r{}",j.grid));
   if !j.estimate{cmd.arg(format!("-l{}",j.tac));}
   let result=cmd.arg(&input).arg(&dest).output().map_err(|e|e.to_string())?;
   if !result.status.success(){return Err(format!("revfix failed ({:?}):\n{}\n{}",result.status.code(),String::from_utf8_lossy(&result.stderr),String::from_utf8_lossy(&result.stdout)))}
   Profile::read(&dest)?
  }
 };
 if output.space()!=reference.space() || output.pcs()!=reference.pcs(){return Err("Engine changed the color space or PCS".into())}
 progress("Validating maximum channel values in all output LUTs...".into());
 let max=output.verify(caps).map_err(|e|format!("{e}\nIf Argyll cannot meet the strict bound, try Custom with an explicit TAC."))?;
 output.clean_stale();
 let name=j.destination.file_stem().unwrap_or_default().to_string_lossy();
 output.describe(&name);
 output.save_new(&j.destination)?;
 Ok(format!("Saved: {}\nVerified maxima (device values): {:.3}% / {:.3}% / {:.3}% / {:.3}%\nTest a printed chart after firing before production.",j.destination.display(),max[0]*100.,max[1]*100.,max[2]*100.,max[3]*100.))
}
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
   match profile.check(){Ok(())=>{self.status=format!("Detected ICC v{} / {} / PCS {}. Confirm physical ink order before rebuilding.",profile.version_major(),profile.space(),profile.pcs());if profile.version_major()==4{self.grid=33;}self.profile=Some(profile);self.destination=Some(p.with_file_name(format!("{}_limited.icc",p.file_stem().unwrap_or_default().to_string_lossy())));}
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
      ui.label("Custom ICC v2 mft2 / v4 mAB (sampled curves). Specify a safe TAC from your print calibration.");
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
