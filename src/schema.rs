table! {
    gallery (message_id) {
        message_id -> Integer,
        gallery_id -> Integer,
        token -> Text,
        title -> Text,
        tags -> Text,
        telegraph -> Text,
        upload_images -> SmallInt,
        publish_date -> Date,
        poll_id -> Text,
        score -> Float,
        votes -> Text,
    }
}

table! {
    images (fileindex) {
        fileindex -> Integer,
        url -> Text,
    }
}

table! {
    user_vote (user_id, poll_id) {
        user_id -> BigInt,
        poll_id -> Integer,
        option -> Integer,
    }
}

allow_tables_to_appear_in_same_query!(
    gallery,
    images,
    user_vote,
);
