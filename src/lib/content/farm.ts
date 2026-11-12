/** Farm catalogues used by the forms. Starting lists only: users can type their own values. */

export const POULTRY = ['broiler', 'layer', 'cockerel', 'noiler', 'turkey', 'guinea_fowl', 'duck', 'goose', 'other_poultry'];
export const LIVESTOCK = ['goat', 'sheep', 'pig', 'cattle', 'rabbit', 'snail', 'other_livestock'];
export const FISH = ['catfish', 'tilapia', 'other_fish'];
export const SPECIES = [...POULTRY, ...LIVESTOCK, ...FISH];

export type Group = 'poultry' | 'livestock' | 'fish';
export const groupOf = (s: string): Group => (POULTRY.includes(s) ? 'poultry' : FISH.includes(s) ? 'fish' : 'livestock');

/** What to call the animals in a batch. */
export const countNoun = (species: string) => (groupOf(species) === 'poultry' ? 'birds' : groupOf(species) === 'fish' ? 'fish' : species === 'snail' ? 'snails' : 'animals');
export const codePrefix: Record<string, string> = { broiler: 'BR', layer: 'LY', cockerel: 'CK', noiler: 'NO', turkey: 'TK', guinea_fowl: 'GF', duck: 'DK', goose: 'GS', other_poultry: 'PL',
  goat: 'GT', sheep: 'SH', pig: 'PG', cattle: 'CT', rabbit: 'RB', snail: 'SN', other_livestock: 'LV', catfish: 'CF', tilapia: 'TL', other_fish: 'FS' };

export const LIVESTOCK_COSTS = ['Feed', 'Vaccine', 'Medication', 'Veterinary visit', 'Litter / bedding', 'Labour', 'Fuel / power', 'Water', 'Transport', 'Other'];

export const CROPS = ['Cassava', 'Yam', 'Cocoyam', 'Sweet potato', 'Irish potato', 'Maize', 'Rice', 'Sorghum', 'Millet', 'Oil palm', 'Coconut', 'Groundnut', 'Soybean',
  'Okra', 'Pepper', 'Tomato', 'Garden egg', 'Fluted pumpkin (ugu)', 'Waterleaf', 'Bitter leaf', 'African spinach', 'Cucumber', 'Amaranth',
  'Plantain', 'Banana', 'Pineapple', 'Orange', 'Mango', 'Pawpaw', 'Avocado'];

export const ACTIVITY_TYPES = ['land_clearing', 'land_preparation', 'ploughing', 'ridging', 'planting', 'transplanting', 'fertiliser', 'weeding', 'irrigation',
  'pest_observation', 'disease_observation', 'spraying', 'pruning', 'mulching', 'harvesting', 'post_harvest', 'other'];

export const HARVEST_UNITS = ['kg', 'bag', 'basket', 'tuber', 'bunch', 'crate', 'tonne', 'heap', 'piece'];

export interface ProcessTemplate { stages: string[]; raw: string; rawUnit: string; costs: string[]; outputs: Array<{ type: 'main' | 'byproduct' | 'waste'; name: string; unit: string }> }

/** Typical stages and records for each kind of processing. Stages are labels only; every figure comes from the farmer. */
export const PROCESS_TEMPLATES: Record<string, ProcessTemplate> = {
  palm_oil: { stages: ['Reception', 'Sorting', 'Sterilising', 'Threshing', 'Digesting', 'Pressing', 'Clarifying', 'Storage'], raw: 'Palm fruit bunches', rawUnit: 'bunch',
    costs: ['Milling fee', 'Transport', 'Labour', 'Fuel', 'Water', 'Packaging', 'Other'], outputs: [{ type: 'main', name: 'Palm oil', unit: 'litre' }, { type: 'main', name: 'Palm kernel', unit: 'kg' }, { type: 'byproduct', name: 'Fibre', unit: 'kg' }, { type: 'waste', name: 'Empty bunches', unit: 'kg' }] },
  palm_kernel: { stages: ['Reception', 'Cracking', 'Separation', 'Drying', 'Storage'], raw: 'Palm nuts', rawUnit: 'kg', costs: ['Cracking fee', 'Transport', 'Labour', 'Packaging', 'Other'], outputs: [{ type: 'main', name: 'Palm kernel', unit: 'kg' }, { type: 'byproduct', name: 'Shells', unit: 'kg' }] },
  cassava_garri: { stages: ['Reception', 'Peeling', 'Washing', 'Grating', 'Fermenting', 'Dewatering', 'Sieving', 'Frying', 'Bagging'], raw: 'Cassava roots', rawUnit: 'kg',
    costs: ['Grating fee', 'Transport', 'Labour', 'Firewood / fuel', 'Water', 'Bags', 'Other'], outputs: [{ type: 'main', name: 'Garri', unit: 'kg' }, { type: 'byproduct', name: 'Peels', unit: 'kg' }, { type: 'waste', name: 'Waste', unit: 'kg' }] },
  cassava_fufu: { stages: ['Reception', 'Peeling', 'Soaking', 'Sieving', 'Dewatering', 'Packing'], raw: 'Cassava roots', rawUnit: 'kg', costs: ['Transport', 'Labour', 'Water', 'Packaging', 'Other'], outputs: [{ type: 'main', name: 'Fufu', unit: 'kg' }, { type: 'byproduct', name: 'Peels', unit: 'kg' }] },
  cassava_flour: { stages: ['Reception', 'Peeling', 'Washing', 'Grating', 'Pressing', 'Drying', 'Milling', 'Bagging'], raw: 'Cassava roots', rawUnit: 'kg', costs: ['Milling fee', 'Transport', 'Labour', 'Fuel', 'Bags', 'Other'], outputs: [{ type: 'main', name: 'Cassava flour', unit: 'kg' }, { type: 'byproduct', name: 'Peels', unit: 'kg' }] },
  cassava_starch: { stages: ['Reception', 'Peeling', 'Grating', 'Settling', 'Drying', 'Bagging'], raw: 'Cassava roots', rawUnit: 'kg', costs: ['Transport', 'Labour', 'Water', 'Bags', 'Other'], outputs: [{ type: 'main', name: 'Cassava starch', unit: 'kg' }, { type: 'byproduct', name: 'Fibre', unit: 'kg' }] },
  grain_milling: { stages: ['Reception', 'Drying', 'Cleaning', 'Shelling', 'Milling', 'Bagging', 'Storage'], raw: 'Maize', rawUnit: 'kg', costs: ['Milling fee', 'Transport', 'Labour', 'Fuel', 'Bags', 'Other'], outputs: [{ type: 'main', name: 'Maize flour', unit: 'kg' }, { type: 'byproduct', name: 'Bran', unit: 'kg' }] },
  feed_milling: { stages: ['Receiving ingredients', 'Weighing', 'Mixing', 'Milling', 'Bagging'], raw: 'Maize', rawUnit: 'kg', costs: ['Milling fee', 'Labour', 'Fuel', 'Bags', 'Other'], outputs: [{ type: 'main', name: 'Finished feed', unit: 'kg' }] },
  groundnut: { stages: ['Reception', 'Shelling', 'Roasting', 'Grinding', 'Packing'], raw: 'Groundnut', rawUnit: 'kg', costs: ['Transport', 'Labour', 'Fuel', 'Packaging', 'Other'], outputs: [{ type: 'main', name: 'Groundnut product', unit: 'kg' }, { type: 'byproduct', name: 'Shells', unit: 'kg' }] },
  other: { stages: ['Reception', 'Processing', 'Packing', 'Storage'], raw: '', rawUnit: 'kg', costs: ['Labour', 'Transport', 'Fuel', 'Packaging', 'Other'], outputs: [] },
};
export const PROCESS_TYPES = Object.keys(PROCESS_TEMPLATES);
