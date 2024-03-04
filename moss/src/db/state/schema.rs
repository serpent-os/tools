// @generated automatically by Diesel CLI.

diesel::table! {
    state (id) {
        id -> Integer,
        #[sql_name = "type"]
        type_ -> Text,
        created -> BigInt,
        summary -> Nullable<Text>,
        description -> Nullable<Text>,
    }
}

diesel::table! {
    state_selections (state_id, package_id) {
        state_id -> Integer,
        package_id -> Text,
        explicit -> Bool,
        reason -> Nullable<Text>,
    }
}

diesel::joinable!(state_selections -> state (state_id));

diesel::allow_tables_to_appear_in_same_query!(state, state_selections,);
