slint::include_modules!();
use std::rc::Rc;

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    ui.on_request_text_edited({
        let ui_handle = ui.as_weak();
        move |text, text_type| {
            let ui = ui_handle.unwrap();
            if text_type == "queries" {
                ui.set_queries(text);
                // println!("queries -> {}", ui.get_queries());
            } else if text_type == "schema" {
                ui.set_schema(text);
                // println!("schema -> {}", ui.get_schema());
            }

            let queries = ui.get_queries();
            let schema = ui.get_schema();
            let errors = match sqlshield::validate_query(queries.as_str(), schema.as_str()) {
                Ok(errors) => errors,
                Err(err) => vec![err],
            };

            let model: Rc<slint::VecModel<slint::StandardListViewItem>> =
                Rc::from(slint::VecModel::from(
                    errors
                        .into_iter()
                        .map(|e| slint::StandardListViewItem::from(slint::SharedString::from(e)))
                        .collect::<Vec<slint::StandardListViewItem>>(),
                ));

            ui.set_errors(slint::ModelRc::from(model));
        }
    });

    ui.run()
}
