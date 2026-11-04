import { describe, expect, it } from 'vitest';
import { fixtureTree } from './fixtures';
import { MockApi } from './MockApi';

const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3]);

describe('MockApi.savePicture', () => {
  it('records_the_picture', async () => {
    const api = new MockApi(fixtureTree(5));
    await expect(api.savePicture('C:/Mock/Whole tree.png', PNG)).resolves.toEqual({ bytes: PNG.length });
    expect(api.savedPictures).toHaveLength(1);
    expect(api.savedPictures[0].dest).toBe('C:/Mock/Whole tree.png');
  });

  it('rejects_an_empty_path_and_bytes_that_are_not_a_png_with_a_plain_error', async () => {
    const api = new MockApi(fixtureTree(5));
    await expect(api.savePicture('', PNG)).rejects.toMatchObject({ code: 'invalid' });
    await expect(api.savePicture('C:/x.png', new Uint8Array([1, 2, 3]))).rejects.toMatchObject({ code: 'invalid' });
    expect(api.savedPictures).toHaveLength(0);
  });

  it('accepts_an_svg_and_requires_a_png_or_svg_ending_that_matches_the_content', async () => {
    const api = new MockApi(fixtureTree(5));
    const svg = new TextEncoder().encode('<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>');
    await expect(api.savePicture('C:/Mock/Chart.SVG', svg)).resolves.toEqual({ bytes: svg.length });
    await expect(api.savePicture('C:/Mock/Chart.png', svg)).rejects.toMatchObject({ code: 'invalid' });
    await expect(api.savePicture('C:/Mock/Chart.svg', PNG)).rejects.toMatchObject({ code: 'invalid' });
    await expect(api.savePicture('C:/Mock/Chart', PNG)).rejects.toMatchObject({ code: 'invalid' });
    await expect(api.savePicture('C:/Mock/Chart.txt', PNG)).rejects.toMatchObject({ code: 'invalid' });
    expect(api.savedPictures).toHaveLength(1);
  });
});
