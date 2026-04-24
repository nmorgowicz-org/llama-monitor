self.addEventListener('fetch', function(event) {
    event.respondWith(
        fetch(event.request).catch(function() {
            return new Response('', {
                status: 503,
                statusText: 'Service Unavailable'
            });
        })
    );
});
