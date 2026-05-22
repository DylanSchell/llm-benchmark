import re

with open('html_equality.rs', 'r') as f:
    content = f.read()

# Replace dashboard tests that check for base HTML elements to check the layout instead
# Pattern: tests that check java_html for DOCTYPE, html, body, head, etc.

# For dashboard tests checking base HTML elements, we need to compare layout.tera instead of dashboard.tera
replacements = [
    # test_dashboard_doctype
    ('fn test_dashboard_doctype()', 'fn test_dashboard_doctype()'),
    # test_dashboard_html_tag
    ('fn test_dashboard_html_tag()', 'fn test_dashboard_html_tag()'),
    # test_dashboard_body_tag
    ('fn test_dashboard_body_tag()', 'fn test_dashboard_body_tag()'),
    # test_dashboard_head_tag
    ('fn test_dashboard_head_tag()', 'fn test_dashboard_head_tag()'),
    # test_dashboard_lang
    ('fn test_dashboard_lang()', 'fn test_dashboard_lang()'),
    # test_dashboard_container
    ('fn test_dashboard_container()', 'fn test_dashboard_container()'),
    # test_dashboard_header
    ('fn test_dashboard_header()', 'fn test_dashboard_header()'),
    # test_dashboard_main
    ('fn test_dashboard_main()', 'fn test_dashboard_main()'),
    # test_dashboard_footer_div
    ('fn test_dashboard_footer_div()', 'fn test_dashboard_footer_div()'),
    # test_dashboard_nav
    ('fn test_dashboard_nav()', 'fn test_dashboard_nav()'),
    # test_dashboard_css_link
    ('fn test_dashboard_css_link()', 'fn test_dashboard_css_link()'),
    # test_dashboard_htmx_script
    ('fn test_dashboard_htmx_script()', 'fn test_dashboard_htmx_script()'),
    # test_dashboard_alpine_script
    ('fn test_dashboard_alpine_script()', 'fn test_dashboard_alpine_script()'),
    # test_dashboard_htmx_sse_script
    ('fn test_dashboard_htmx_sse_script()', 'fn test_dashboard_htmx_sse_script()'),
    # test_dashboard_inline_style
    ('fn test_dashboard_inline_style()', 'fn test_dashboard_inline_style()'),
    # test_dashboard_sortable_table_css
    ('fn test_dashboard_sortable_table_css()', 'fn test_dashboard_sortable_table_css()'),
    # test_dashboard_stats_grid_css
    ('fn test_dashboard_stats_grid_css()', 'fn test_dashboard_stats_grid_css()'),
    # test_dashboard_stat_card_css
    ('fn test_dashboard_stat_card_css()', 'fn test_dashboard_stat_card_css()'),
    # test_dashboard_card_css
    ('fn test_dashboard_card_css()', 'fn test_dashboard_card_css()'),
    # test_dashboard_button_css
    ('fn test_dashboard_button_css()', 'fn test_dashboard_button_css()'),
    # test_dashboard_nav_links
    ('fn test_dashboard_nav_links()', 'fn test_dashboard_nav_links()'),
    # test_dashboard_title
    ('fn test_dashboard_title()', 'fn test_dashboard_title()'),
    # test_dashboard_h1_title
    ('fn test_dashboard_h1_title()', 'fn test_dashboard_h1_title()'),
    # test_dashboard_h2_title
    ('fn test_dashboard_h2_title()', 'fn test_dashboard_h2_title()'),
    # test_dashboard_h3_titles
    ('fn test_dashboard_h3_titles()', 'fn test_dashboard_h3_titles()'),
    # test_dashboard_footer
    ('fn test_dashboard_footer()', 'fn test_dashboard_footer()'),
    # test_dashboard_meta_charset
    ('fn test_dashboard_meta_charset()', 'fn test_dashboard_meta_charset()'),
    # test_dashboard_meta_viewport
    ('fn test_dashboard_meta_viewport()', 'fn test_dashboard_meta_viewport()'),
    # test_dashboard_inline_js
    ('fn test_dashboard_inline_js()', 'fn test_dashboard_inline_js()'),
    # test_dashboard_css_link
    ('fn test_dashboard_css_link()', 'fn test_dashboard_css_link()'),
]

# For tests that check base HTML elements in dashboard, change to check layout.tera
# We need to find the specific test functions and change the rust_template path

print("Done - manual fix needed")
