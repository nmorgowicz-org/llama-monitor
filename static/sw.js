// Service worker disabled for development - prevents caching of updated JS
// Enable in production with proper cache versioning
self.addEventListener('fetch', function(event) {
    // Pass through all requests - no caching
    event.respondWith(fetch(event.request));
});
