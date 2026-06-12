#!/usr/bin/env node

import { Command } from 'commander';
import { registerGenerateCommand } from '../lib/commands/generate.js';
import { registerDeployCommand } from '../lib/commands/deploy.js';
import { registerVerifyCommand } from '../lib/commands/verify.js';
import { registerEstimateCommand } from '../lib/commands/estimate.js';
import { registerInitCommand } from '../lib/commands/init.js';

const program = new Command();

program
  .name('defai')
  .description('DEFAI Element CLI - Manage HEIR.ES inheritance smart contracts')
  .version('1.0.0');

registerGenerateCommand(program);
registerDeployCommand(program);
registerVerifyCommand(program);
registerEstimateCommand(program);
registerInitCommand(program);

program.parse();
