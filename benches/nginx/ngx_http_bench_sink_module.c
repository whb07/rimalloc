// Upstream-only sink: respond after the entire body has been consumed.
#include <ngx_config.h>
#include <ngx_core.h>
#include <ngx_http.h>

static void body_done(ngx_http_request_t* r) {
    off_t bytes = 0;
    for (ngx_chain_t* cl = r->request_body->bufs; cl; cl = cl->next) bytes += ngx_buf_size(cl->buf);
    if (bytes != r->headers_in.content_length_n) {
        ngx_http_finalize_request(r, NGX_HTTP_BAD_REQUEST);
        return;
    }
    ngx_table_elt_t* header = ngx_list_push(&r->headers_out.headers);
    u_char* value = ngx_pnalloc(r->pool, NGX_OFF_T_LEN);
    if (!header || !value) { ngx_http_finalize_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR); return; }
    header->hash = 1;
    ngx_str_set(&header->key, "X-Received-Bytes");
    header->value.data = value;
    header->value.len = ngx_sprintf(value, "%O", bytes) - value;
    r->headers_out.status = NGX_HTTP_NO_CONTENT;
    r->headers_out.content_length_n = 0;
    ngx_http_finalize_request(r, ngx_http_send_header(r));
}
static ngx_int_t sink(ngx_http_request_t* r) {
    if (r->method != NGX_HTTP_POST || r->headers_in.content_length_n < 0) return NGX_HTTP_NOT_ALLOWED;
    ngx_int_t rc = ngx_http_read_client_request_body(r, body_done);
    return rc >= NGX_HTTP_SPECIAL_RESPONSE ? rc : NGX_DONE;
}
static char* configure(ngx_conf_t* cf, ngx_command_t* cmd, void* conf) {
    (void)cmd; (void)conf;
    ngx_http_core_loc_conf_t* location = ngx_http_conf_get_module_loc_conf(cf, ngx_http_core_module);
    location->handler = sink;
    return NGX_CONF_OK;
}
static ngx_command_t commands[] = {
    { ngx_string("bench_sink"), NGX_HTTP_LOC_CONF | NGX_CONF_NOARGS, configure, 0, 0, NULL },
    ngx_null_command
};
static ngx_http_module_t context = { NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL };
ngx_module_t ngx_http_bench_sink_module = {
    NGX_MODULE_V1, &context, commands, NGX_HTTP_MODULE,
    NULL, NULL, NULL, NULL, NULL, NULL, NULL, NGX_MODULE_V1_PADDING
};
