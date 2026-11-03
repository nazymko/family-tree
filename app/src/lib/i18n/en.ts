/**
 * The English catalogue. It is the source of truth: its shape is the schema every other language must
 * follow (see `Widen` in types.ts). An entry with an `other` form is a count message used with plural().
 */
export const en = {
  common: {
    save: 'Save',
    cancel: 'Cancel',
    close: 'Close',
    people: { one: '{n} person', other: '{n} people' },
  },
  settings: {
    title: 'Settings',
    dateOrder: 'Date order',
    dateOrderHint: 'How 03/04/1901 is read when you type it',
    dayMonthYear: 'Day, month, year (3 April 1901)',
    monthDayYear: 'Month, day, year (March 4, 1901)',
    nameOrder: 'Name order',
    givenFirst: 'Given name first (Anna Schmidt)',
    surnameFirst: 'Surname first (Schmidt, Anna)',
    livingThreshold: 'Living threshold',
    livingThresholdHint: 'People born more than {age} years ago with no other information are treated as deceased',
    homePerson: 'Home person',
    homePersonHint: 'The person the family view opens on.',
    language: 'Language',
    languageHint: 'The language of menus, buttons and messages.',
    languageSystem: 'Same as the system',
    theme: 'Theme',
    themeLight: 'Light',
    themeDark: 'Dark',
    themeSystem: 'System',
    backups: 'Backups to keep',
  },
};
