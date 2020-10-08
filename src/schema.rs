table! {
    gallery (gallery_id) {
        gallery_id -> Integer,
        title -> Text,
        tags -> Text,
        upload_images -> SmallInt,
        publish_date -> Date,
        message_id -> Integer,
        poll_id -> Text,
        score -> Float,
    }
}

table! {
    images (gallery_id, number) {
        gallery_id -> Integer,
        number -> Integer,
        url -> Text,
    }
}

table! {
    users (user_id) {
        user_id -> Integer,
        warn -> SmallInt,
    }
}

allow_tables_to_appear_in_same_query!(gallery, images, users,);
