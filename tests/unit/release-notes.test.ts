import { describe, expect, it } from 'vitest';
import { releaseNotesToText } from '@/main/update/release-notes';

describe('releaseNotesToText', () => {
  it('converts headings and lists to readable plain text', () => {
    const html =
      '<h2>Features</h2><ul><li>Add color picker and enhance overlay capture workflows</li><li>Second change</li></ul>';

    expect(releaseNotesToText(html)).toBe(
      'Features\n\n- Add color picker and enhance overlay capture workflows\n\n- Second change'
    );
  });

  it('keeps link text and drops link markup', () => {
    const html =
      '<li>Fix crash (<a class="issue-link" data-error-text="Failed to load title" href="https://github.com/Porabuild/Poratake/pull/8">#8</a>)</li>';

    expect(releaseNotesToText(html)).toBe('- Fix crash (#8)');
  });

  it('decodes named and numeric entities', () => {
    const html =
      '<p>Tom &amp; Jerry &lt;3 &quot;quotes&quot; &#8212; &#x2026;</p>';

    expect(releaseNotesToText(html)).toBe('Tom & Jerry <3 "quotes" — …');
  });

  it('removes script and style blocks entirely', () => {
    const html =
      '<p>Notes</p><script>alert(1)</script><style>p{color:red}</style>';

    expect(releaseNotesToText(html)).toBe('Notes');
  });

  it('leaves markdown notes untouched', () => {
    const markdown = "## What's Changed\n* Something new by @user in #8";

    expect(releaseNotesToText(markdown)).toBe(markdown);
  });

  it('collapses excess blank lines and trims', () => {
    const html = '\n<h2>Title</h2>\n\n\n<p>Body</p>\n';

    expect(releaseNotesToText(html)).toBe('Title\n\nBody');
  });
});
