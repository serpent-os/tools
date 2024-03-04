// @generated automatically by Diesel CLI.

diesel::table! {
    layout (id) {
        id -> Integer,
        package_id -> Text,
        uid -> Integer,
        gid -> Integer,
        mode -> Integer,
        tag -> Integer,
        entry_type -> Text,
        entry_value1 -> Nullable<Text>,
        entry_value2 -> Nullable<Text>,
    }
}
