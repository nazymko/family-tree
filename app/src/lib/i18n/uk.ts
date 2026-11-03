import type { Messages } from './locale.svelte';

/** Ukrainian. The type makes a missing or extra key a compile error. */
export const uk: Messages = {
  common: {
    save: 'Зберегти',
    cancel: 'Скасувати',
    close: 'Закрити',
    people: { one: '{n} особа', few: '{n} особи', many: '{n} осіб', other: '{n} особи' },
  },
  settings: {
    title: 'Налаштування',
    dateOrder: 'Порядок запису дати',
    dateOrderHint: 'Як читається 03/04/1901, коли Ви вводите дату',
    dayMonthYear: 'День, місяць, рік (3 квітня 1901)',
    monthDayYear: 'Місяць, день, рік (4 березня 1901)',
    nameOrder: 'Порядок запису імені',
    givenFirst: 'Спочатку ім’я (Анна Шмідт)',
    surnameFirst: 'Спочатку прізвище (Шмідт, Анна)',
    livingThreshold: 'Межа для живих',
    livingThresholdHint: 'Особи, про яких відомо лише, що вони народилися понад {age} р. тому, вважаються померлими',
    homePerson: 'Головна особа',
    homePersonHint: 'Особа, з якої відкривається родинний вигляд.',
    language: 'Мова',
    languageHint: 'Мова меню, кнопок і повідомлень.',
    languageSystem: 'Як у системі',
    theme: 'Тема',
    themeLight: 'Світла',
    themeDark: 'Темна',
    themeSystem: 'Як у системі',
    backups: 'Скільки резервних копій зберігати',
  },
};
