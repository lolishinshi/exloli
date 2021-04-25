table! {
    gallery (gallery_id) {
        gallery_id -> Integer,
        token -> Text,
        title -> Text,
        tags -> Text,
        telegraph -> Text,
        upload_images -> SmallInt,
        publish_date -> Date,
        message_id -> Integer,
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

allow_tables_to_appear_in_same_query!(gallery, images,);
