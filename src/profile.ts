export const requiredProfileFields = ["name", "email", "phone", "city", "state", "country"] as const;

export function isProfileComplete(profile: Record<(typeof requiredProfileFields)[number], string>): boolean {
  return requiredProfileFields.every((field) => profile[field].trim().length > 0);
}
