// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(mysql_type(name = "Enum"))]
    pub struct CellsRadioEnum;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(mysql_type(name = "Enum"))]
    pub struct LastUpdatesUpdateTypeEnum;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::CellsRadioEnum;

    cells (radio, mcc, net, area, cell) {
        #[max_length = 4]
        radio -> CellsRadioEnum,
        mcc -> Unsigned<Smallint>,
        net -> Unsigned<Smallint>,
        area -> Unsigned<Smallint>,
        cell -> Unsigned<Integer>,
        unit -> Nullable<Unsigned<Smallint>>,
        lon -> Float,
        lat -> Float,
        cell_range -> Unsigned<Integer>,
        samples -> Unsigned<Integer>,
        changeable -> Bool,
        created -> Datetime,
        updated -> Datetime,
        average_signal -> Nullable<Smallint>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::LastUpdatesUpdateTypeEnum;

    last_updates (update_type) {
        #[max_length = 4]
        update_type -> LastUpdatesUpdateTypeEnum,
        value -> Datetime,
    }
}

diesel::allow_tables_to_appear_in_same_query!(cells, last_updates,);
