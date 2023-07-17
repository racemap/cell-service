use std::{env, error::Error};

use diesel::RunQueryDsl;

use super::db::establish_connection;

pub fn load_data(input_path: String) -> Result<(), Box<dyn Error>> {
    let full_path = match input_path.starts_with("/") {
        true => input_path,
        false => {
            let mut path = env::current_dir()?;
            path.push(input_path);
            let path_full = path.clone();
            String::from(path_full.to_str().unwrap())
        }
    };
    let connection = &mut establish_connection();

    let res = diesel::sql_query(format!("
    LOAD DATA INFILE {:?}
    REPLACE INTO TABLE cells
    FIELDS TERMINATED BY ','
    LINES TERMINATED BY '\r\n'
    IGNORE 1 LINES
    (radio, mcc, net, area, cell, @unit, lon, lat, cell_range, samples, changeable, @created, @updated, @average_signal)
    SET
    unit = NULLIF(@unit, ''),
    average_signal = NULLIF(@average_signal, ''),
    created = FROM_UNIXTIME(@created),
    updated = FROM_UNIXTIME(@updated);", full_path)).execute(connection);

    match res {
        Ok(writes) => println!("Success: {:?} writes.", writes),
        Err(e) => println!("Error: {:?}", e),
    }
    Ok(())
}
