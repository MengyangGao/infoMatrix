#ifndef InfoMatrixCore_h
#define InfoMatrixCore_h

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

char *infomatrix_core_health_json(void);
char *infomatrix_core_meta_json(void);
char *infomatrix_core_default_db_path_json(void);
char *infomatrix_core_list_feeds_json(const char *input);
char *infomatrix_core_list_groups_json(const char *input);
char *infomatrix_core_create_group_json(const char *input);
char *infomatrix_core_update_feed_json(const char *input);
char *infomatrix_core_update_feed_group_json(const char *input);
char *infomatrix_core_delete_feed_json(const char *input);
char *infomatrix_core_add_subscription_json(const char *input);
char *infomatrix_core_subscribe_input_json(const char *input);
char *infomatrix_core_discover_site_json(const char *input);
char *infomatrix_core_export_opml_json(const char *input);
char *infomatrix_core_import_opml_json(const char *input);
char *infomatrix_core_refresh_feed_json(const char *input);
char *infomatrix_core_refresh_due_json(const char *input);
char *infomatrix_core_list_items_json(const char *input);
char *infomatrix_core_list_entries_json(const char *input);
char *infomatrix_core_item_counts_json(const char *input);
char *infomatrix_core_get_entry_json(const char *input);
char *infomatrix_core_create_entry_json(const char *input);
char *infomatrix_core_fetch_fulltext_json(const char *input);
char *infomatrix_core_patch_item_state_json(const char *input);
char *infomatrix_core_get_global_notification_settings_json(const char *input);
char *infomatrix_core_update_global_notification_settings_json(const char *input);
char *infomatrix_core_get_feed_notification_settings_json(const char *input);
char *infomatrix_core_update_feed_notification_settings_json(const char *input);
char *infomatrix_core_list_pending_notification_events_json(const char *input);
char *infomatrix_core_ack_notification_events_json(const char *input);
char *infomatrix_core_list_sync_events_json(const char *input);
char *infomatrix_core_ack_sync_events_json(const char *input);
void infomatrix_core_free_string(char *ptr);

#endif /* InfoMatrixCore_h */
