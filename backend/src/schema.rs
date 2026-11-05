// Written by hand to match migrations/, check with `diesel print-schema`.

diesel::table! {
    users (id) {
        id -> Int8,
        username -> Text,
        email -> Text,
        password_hash -> Text,
        coins -> Int8,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    sessions (id) {
        id -> Int8,
        user_id -> Int8,
        token_hash -> Bytea,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
    }
}

diesel::table! {
    owned_cards (id) {
        id -> Int8,
        user_id -> Int8,
        card -> Text,
        foil -> Nullable<Text>,
        obtained_at -> Timestamptz,
    }
}

diesel::table! {
    boosters (id) {
        id -> Int8,
        user_id -> Int8,
        collection -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    streaks (user_id) {
        user_id -> Int8,
        day -> Int4,
        best -> Int4,
        last_claim -> Nullable<Date>,
    }
}

diesel::table! {
    topups (id) {
        id -> Int8,
        user_id -> Int8,
        pack -> Text,
        coins -> Int8,
        price_cents -> Int4,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    trades (id) {
        id -> Int8,
        from_id -> Int8,
        to_id -> Int8,
        state -> Text,
        version -> Int4,
        from_coins -> Int8,
        to_coins -> Int8,
        from_sealed -> Nullable<Int4>,
        to_sealed -> Nullable<Int4>,
        edited_by -> Nullable<Int8>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    trade_lines (id) {
        id -> Int8,
        trade_id -> Int8,
        side -> Text,
        card -> Text,
        foil -> Bool,
        count -> Int4,
    }
}

diesel::joinable!(trade_lines -> trades (trade_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(topups -> users (user_id));
diesel::joinable!(owned_cards -> users (user_id));
diesel::joinable!(boosters -> users (user_id));
diesel::joinable!(streaks -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    users,
    sessions,
    owned_cards,
    boosters,
    streaks,
    topups,
    trades,
    trade_lines
);
