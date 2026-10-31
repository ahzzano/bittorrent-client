use eframe::egui;

fn main() -> eframe::Result{
    let options = eframe::NativeOptions::default();
    eframe::run_ui_native("BitTorrent Client", options, |ui, _frame| {
        egui::CentralPanel::default().show(ui, |ui|{
            ui.label("Hello World");
        });
    })
}
