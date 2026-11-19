// Development only: loaded with a dynamic import from App.svelte inside `import.meta.env.DEV`, so a production
// build never contains it. Everything here is invented.
import type { Api } from '../api/api';
import { nav } from '../state/nav.svelte';

/**
 * `?route=asRecorded`: an invented man, «Федір Петрович Вигаданенко», with his name as a metric book of 1885 writes
 * it («Ѳеодоръ Петровъ Вигаданенко», cited to the book); opens his page.
 */
export async function showAsRecorded(api: Api): Promise<void> {
  const silent = { silent: true };
  const man = (
    await api.apply({ type: 'createPerson', draft: { given: 'Федір', patronymic: 'Петрович', surnames: ['Вигаданенко'], sex: 'male', birthDateRaw: '12 Mar 1885' } }, silent)
  ).focus!;
  const add = async (person: string, given: string, patronymic: string, surname: string, page: string, text: string) => {
    const r = await api.apply(
      { type: 'saveName', person, name: { kind: 'as_recorded', preferred: false, private: false, given, patronymic, surnames: [surname], dateRaw: '1885' } },
      silent,
    );
    const name = r.touched[r.touched.length - 1];
    await api.apply(
      {
        type: 'addCitation',
        target: { kind: 'name', id: name },
        citation: { source: { title: 'Метрична книга Вигаданої церкви, 1885' }, page, dateRaw: '14 Mar 1885', text },
      },
      silent,
    );
  };
  await add(man, 'Ѳеодоръ', 'Петровъ', 'Вигаданенко', 'арк. 12 зв., № 34', 'Ѳеодоръ, сынъ крестьянина Петра Вигаданенка');
  nav.go({ name: 'person', id: man });
}
