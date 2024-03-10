// @generated automatically by Diesel CLI.

diesel::table! {
    meta (package) {
        package -> Text,
        name -> Text,
        version_identifier -> Text,
        source_release -> Integer,
        build_release -> Integer,
        architecture -> Text,
        summary -> Text,
        description -> Text,
        source_id -> Text,
        homepage -> Text,
        uri -> Nullable<Text>,
        hash -> Nullable<Text>,
        download_size -> Nullable<BigInt>,
    }
}

diesel::table! {
    meta_dependencies (package, dependency) {
        package -> Text,
        dependency -> Text,
    }
}

diesel::table! {
    meta_licenses (package, license) {
        package -> Text,
        license -> Text,
    }
}

diesel::table! {
    meta_providers (package, provider) {
        package -> Text,
        provider -> Text,
    }
}

diesel::joinable!(meta_dependencies -> meta (package));
diesel::joinable!(meta_licenses -> meta (package));
diesel::joinable!(meta_providers -> meta (package));

diesel::allow_tables_to_appear_in_same_query!(meta, meta_dependencies, meta_licenses, meta_providers,);
