/** Who sells FarmBuddy licence keys. Shown on the activation screen and in Settings → Licence. */
export const VENDOR = {
  name: 'TechAlpha Hub',
  whatsapp: '+2347066155981',
  phoneDisplay: '+234 706 615 5981',
  email: 'info@techalphahub.online',
  prices: ['₦30,000 per year', '₦3,000 per month'],
};

export function keyRequestText(installId: string, businessName = ''): string {
  return [
    `Hello ${VENDOR.name}, please send me an FarmBuddy licence key.`,
    '',
    `Install ID: ${installId}`,
    `Business name: ${businessName}`,
    'Plan (yearly ₦30,000 or monthly ₦3,000): ',
  ].join('\n');
}
