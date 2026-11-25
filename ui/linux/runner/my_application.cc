// Koalino's GTK application: creates the window that hosts the Flutter view, and keeps the
// window's title bar in the desktop's light or dark style.
//
// GTK 3 draws the title bar itself on GNOME (Wayland) but ignores the desktop's dark style, which
// only GTK 4 and libadwaita follow, so the bar stayed white over a dark app. The window now reads
// the style from the XDG desktop portal (GNOME, KDE and others) and follows its changes. The
// Flutter content already follows the system style (`ThemeMode.system`).
#include "my_application.h"

#include <flutter_linux/flutter_linux.h>

#include "flutter/generated_plugin_registrant.h"

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
  // The session bus, while the portal's style changes are followed; null otherwise.
  GDBusConnection* session_bus;
  // The subscription to the portal's SettingChanged signal; 0 when there is none.
  guint color_scheme_subscription;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

// The XDG desktop portal's settings, where the desktop publishes its light or dark style.
static constexpr char kPortalBusName[] = "org.freedesktop.portal.Desktop";
static constexpr char kPortalObjectPath[] = "/org/freedesktop/portal/desktop";
static constexpr char kPortalSettings[] = "org.freedesktop.portal.Settings";
static constexpr char kAppearanceNamespace[] = "org.freedesktop.appearance";
static constexpr char kColorSchemeKey[] = "color-scheme";
// The portal's value for "prefer dark" (0 is no preference, 2 prefer light).
static constexpr guint32 kPreferDark = 1;

// Asks GTK for the dark or light variant of its theme, which styles the window's title bar.
//
// Inputs: The portal's color-scheme value, possibly wrapped in one or two variants (the older
// Read method wraps it twice). A value of another type is ignored.
// Side effects: Sets the process-wide `gtk-application-prefer-dark-theme` setting.
static void apply_color_scheme(GVariant* value) {
  g_autoptr(GVariant) unwrapped = g_variant_ref(value);
  while (g_variant_is_of_type(unwrapped, G_VARIANT_TYPE_VARIANT)) {
    GVariant* inner = g_variant_get_variant(unwrapped);
    g_variant_unref(unwrapped);
    unwrapped = inner;
  }
  if (!g_variant_is_of_type(unwrapped, G_VARIANT_TYPE_UINT32)) {
    return;
  }
  GtkSettings* settings = gtk_settings_get_default();
  if (settings == nullptr) {
    return;
  }
  const gboolean prefer_dark = g_variant_get_uint32(unwrapped) == kPreferDark;
  g_object_set(settings, "gtk-application-prefer-dark-theme", prefer_dark,
               nullptr);
}

// Applies the style the portal answered with, once at start-up.
//
// Inputs: The finished ReadOne call. Outputs: None; without a portal (or an older one without
// ReadOne) GTK keeps its default light style. Side effects: See `apply_color_scheme`.
static void color_scheme_read_cb(GObject* source, GAsyncResult* result,
                                 gpointer user_data) {
  g_autoptr(GError) error = nullptr;
  g_autoptr(GVariant) reply = g_dbus_connection_call_finish(
      G_DBUS_CONNECTION(source), result, &error);
  if (reply == nullptr) {
    g_debug("The desktop's color scheme could not be read: %s",
            error->message);
    return;
  }
  g_autoptr(GVariant) value = nullptr;
  g_variant_get(reply, "(v)", &value);
  apply_color_scheme(value);
}

// Follows the desktop switching between light and dark while Koalino runs.
//
// Inputs: A SettingChanged signal: (namespace, key, value). Other settings are ignored.
// Side effects: See `apply_color_scheme`.
static void color_scheme_changed_cb(GDBusConnection* connection,
                                    const gchar* sender_name,
                                    const gchar* object_path,
                                    const gchar* interface_name,
                                    const gchar* signal_name,
                                    GVariant* parameters, gpointer user_data) {
  if (!g_variant_is_of_type(parameters, G_VARIANT_TYPE("(ssv)"))) {
    return;
  }
  const gchar* setting_namespace = nullptr;
  const gchar* key = nullptr;
  g_autoptr(GVariant) value = nullptr;
  g_variant_get(parameters, "(&s&sv)", &setting_namespace, &key, &value);
  if (g_strcmp0(setting_namespace, kAppearanceNamespace) == 0 &&
      g_strcmp0(key, kColorSchemeKey) == 0) {
    apply_color_scheme(value);
  }
}

// Reads the desktop's style and subscribes to its changes.
//
// Inputs: The application, which keeps the bus and the subscription until it is disposed.
// Outputs: None; without a session bus the title bar keeps GTK's default style.
// Side effects: One asynchronous D-Bus call to the portal, and a signal subscription.
static void follow_desktop_color_scheme(MyApplication* self) {
  g_autoptr(GError) error = nullptr;
  self->session_bus = g_bus_get_sync(G_BUS_TYPE_SESSION, nullptr, &error);
  if (self->session_bus == nullptr) {
    g_debug("No session bus for the desktop's color scheme: %s",
            error->message);
    return;
  }
  self->color_scheme_subscription = g_dbus_connection_signal_subscribe(
      self->session_bus, kPortalBusName, kPortalSettings, "SettingChanged",
      kPortalObjectPath, kAppearanceNamespace, G_DBUS_SIGNAL_FLAGS_NONE,
      color_scheme_changed_cb, nullptr, nullptr);
  g_dbus_connection_call(
      self->session_bus, kPortalBusName, kPortalObjectPath, kPortalSettings,
      "ReadOne", g_variant_new("(ss)", kAppearanceNamespace, kColorSchemeKey),
      G_VARIANT_TYPE("(v)"), G_DBUS_CALL_FLAGS_NONE, -1, nullptr,
      color_scheme_read_cb, nullptr);
}

// Called when first Flutter frame received.
static void first_frame_cb(MyApplication* self, FlView* view) {
  gtk_widget_show(gtk_widget_get_toplevel(GTK_WIDGET(view)));
}

// Gives `window` Koalino's icon (#165), for title bars and task bars when no installed desktop
// entry provides one (the tarball). The icon is `data/koalino.png` next to the executable, which
// the bundle's install step copies there.
//
// Inputs: The application window. Outputs: None; a missing or unreadable icon leaves GTK's default.
// Side effects: Reads the executable's link and the icon file.
static void set_window_icon(GtkWindow* window) {
  g_autofree gchar* executable = g_file_read_link("/proc/self/exe", nullptr);
  if (executable == nullptr) return;
  g_autofree gchar* bundle = g_path_get_dirname(executable);
  g_autofree gchar* icon = g_build_filename(bundle, "data", "koalino.png", nullptr);
  gtk_window_set_icon_from_file(window, icon, nullptr);
}

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);

  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));
  gtk_window_set_default_size(window, 1280, 720);

  // If you want to use a header bar, uncomment the following lines.
  // Header bars are commonly used in GNOME based desktop environments and
  // allow additional widgets to be added to the title bar.
  // See https://docs.gtk.org/gtk3/class.HeaderBar.html
  // GtkHeaderBar* header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
  // gtk_widget_show(GTK_WIDGET(header_bar));
  // gtk_header_bar_set_show_close_button(header_bar, TRUE);
  // gtk_window_set_titlebar(window, GTK_WIDGET(header_bar));

  gtk_window_set_title(window, "Koalino");
  set_window_icon(window);
  follow_desktop_color_scheme(self);

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  fl_dart_project_set_dart_entrypoint_arguments(
      project, self->dart_entrypoint_arguments);

  FlView* view = fl_view_new(project);
  GdkRGBA background_color;
  // Background defaults to black, override it here if necessary, e.g. #00000000
  // for transparent.
  gdk_rgba_parse(&background_color, "#000000");
  fl_view_set_background_color(view, &background_color);
  gtk_widget_show(GTK_WIDGET(view));
  gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(view));

  // Show the window when Flutter renders.
  // Requires the view to be realized so we can start rendering.
  g_signal_connect_swapped(view, "first-frame", G_CALLBACK(first_frame_cb),
                           self);
  gtk_widget_realize(GTK_WIDGET(view));

  fl_register_plugins(FL_PLUGIN_REGISTRY(view));

  gtk_widget_grab_focus(GTK_WIDGET(view));
}

// Implements GApplication::local_command_line.
static gboolean my_application_local_command_line(GApplication* application,
                                                  gchar*** arguments,
                                                  int* exit_status) {
  MyApplication* self = MY_APPLICATION(application);
  // Strip out the first argument as it is the binary name.
  self->dart_entrypoint_arguments = g_strdupv(*arguments + 1);

  g_autoptr(GError) error = nullptr;
  if (!g_application_register(application, nullptr, &error)) {
    g_warning("Failed to register: %s", error->message);
    *exit_status = 1;
    return TRUE;
  }

  g_application_activate(application);
  *exit_status = 0;

  return TRUE;
}

// Implements GApplication::startup.
static void my_application_startup(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application startup.

  G_APPLICATION_CLASS(my_application_parent_class)->startup(application);
}

// Implements GApplication::shutdown.
static void my_application_shutdown(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application shutdown.

  G_APPLICATION_CLASS(my_application_parent_class)->shutdown(application);
}

// Implements GObject::dispose.
static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  if (self->session_bus != nullptr && self->color_scheme_subscription != 0) {
    g_dbus_connection_signal_unsubscribe(self->session_bus,
                                         self->color_scheme_subscription);
    self->color_scheme_subscription = 0;
  }
  g_clear_object(&self->session_bus);
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->local_command_line =
      my_application_local_command_line;
  G_APPLICATION_CLASS(klass)->startup = my_application_startup;
  G_APPLICATION_CLASS(klass)->shutdown = my_application_shutdown;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {}

MyApplication* my_application_new() {
  // Set the program name to the application ID, which helps various systems
  // like GTK and desktop environments map this running application to its
  // corresponding .desktop file. This ensures better integration by allowing
  // the application to be recognized beyond its binary name.
  g_set_prgname(APPLICATION_ID);

  return MY_APPLICATION(g_object_new(my_application_get_type(),
                                     "application-id", APPLICATION_ID, "flags",
                                     G_APPLICATION_NON_UNIQUE, nullptr));
}
