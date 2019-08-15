#! /usr/bin/env python2
# -*- coding: utf-8 -*-
#===============================================================================#
#   DESCRIPTION:
#
#       OPTIONS:  ---
#  REQUIREMENTS:  python2 (version >= 2.7)
#         NOTES:  ---
#        AUTHOR:  Wenping Guo
#         EMAIL:  winpng@gmail.com
#       LICENCE:  GPL version 2 or upper
#       CREATED:  2014-10-22
#===============================================================================#
__VERSION__ = "1.2.3-r15"
__UPDATED__ = '2017-06-30 10:46:42 ybyygu'

import sys
import os
import argparse
import re
import itertools
import subprocess
import logging
import re
import StringIO
import logging.config
import itertools
import random
import time
import yaml

from collections import defaultdict

import pyevolve
import copy
from pyevolve import G1DBinaryString
from pyevolve import GSimpleGA
from pyevolve import DBAdapters
from pyevolve import Consts
from pyevolve import Util
from pyevolve import Scaling
from pyevolve import GPopulation
from pyevolve import Selectors


PBS_FILE = """ #!/usr/bin/env bash
#===============================================================================#
#   DESCRIPTION: pbs script for magnetic moment (MAGMOM) search using GA
#                submit this script with qsub, e.g.
#                > qsub run.pbs -l nodes=1:ppn=12 -N test
#                > bsub run.pbs -J test
#
#                if you want to run on 5 nodes in parallel
#                > qsub -t 1-5 run.pbs -l nodes=1:ppn=12 -N test
#                > bsub -J test[1-5]%5 run.pbs
#
#        AUTHOR:  Wenping Guo (ybyygu)
#         EMAIL:  win.png@gmail.com
#===============================================================================#
# how many slots will be used for parallel?
#+make sure this is consistent with "-t" or "-J" specification.
MAX_SLOTS=5
MAX_JOBS=400

# magcalc itself
MAGCALC_CMDLINE="magcalc.py -r -t template/INCAR 12 5.0"

# change to work directory
# PBS or LFS?
if [[ x$PBS_O_WORKDIR != x ]]; then
    cd "$PBS_O_WORKDIR"
elif [[ x$LS_SUBCWD != x ]]; then
    cd $LS_SUBCWD
fi

# working around a pyinstaller bug for LD_LIBRARY_PATH
export OLD_LIBRARY_PATH=$LD_LIBRARY_PATH

OUTFILE=pbs.out
# parallel or serial mode?
ARRAY_ID=""
if [[ x$PBS_ARRAYID != x ]]; then
    export ARRAY_ID=$PBS_ARRAYID
    echo "parallel running using PBS on node `hostname` ($ARRAY_ID)" >> $OUTFILE
elif [[ x$LSB_JOBINDEX != x ]]; then
    export ARRAY_ID=$LSB_JOBINDEX
    echo "parallel running using LSF on node `hostname` ($ARRAY_ID)" >> $OUTFILE
fi

# run in serial
if [[ x$ARRAY_ID == x ]]; then
    echo 'run in serial mode' >> $OUTFILE
    env MAGCALC_MAX_JOBS=$MAX_JOBS $MAGCALC_CMDLINE
# run in parallel
else
    # on node1 we calculate 1, 6, 11, 16, ...
    # on node2 we calculate 2, 7, 12, 17, ...
    # on nodeX we ....
    new_id=$ARRAY_ID
    while [[ $new_id -le $MAX_JOBS ]]; do
        echo "calculate on slot $new_id" >> $OUTFILE
        env MAGCALC_MAX_JOBS=$MAX_JOBS MAGCALC_SLOT_ID=$new_id $MAGCALC_CMDLINE || exit 0
        let new_id+=$MAX_SLOTS
    done
    # when all jobs have been calculated, we produce the final results
    if [[ $ARRAY_ID == 1 ]]; then
        env MAGCALC_MAX_JOBS=$MAX_JOBS $MAGCALC_CMDLINE
    fi
fi

"""

DEFAULT_CONFIG = """# config file version; do not change
version: {config_version}
# set random_seed to a non-zero integer if you want to resume random searching.
random_seed: {random_seed}
# set brute_force to True if want to explore all possible combinations.
brute_force: False

Genetic Algorithm:
  generations:     100
  population_size: 20
  # mutation_rate: the recommended value is between 0.02~0.20
  mutation_rate:   0.1
  crossover_rate:  0.9
  # elitism will always be kept in the population
  elitism_number:  3

Run Script:
  filename:
    jobs/job.run
"""

JOB_RUN = """#!/usr/bin/env bash
# set vasp running environment here
# source /home-gg/compiler/intel/composer_xe_2011_sp1/bin/compilervars.sh intel64
# source /home-gg/compiler/mpi/openmpi-1.4.4-intel.sh
VASP=~/bin/vasp

cd $XXX_JOBDIR/
# run it and remove unnecessary files if finished correctly
mpirun -np 12 $VASP > vasp.out && rm -f WAVECAR CHG CHGCAR PROCAR DOSCAR vasprun.xml EIGENVAL OUTCAR vasp.out
"""


YAML_CONF = """
version: 1
disable_existing_loggers: true

root:
  level: !!python/name:logging.NOTSET
  handlers: [logfile, console]

handlers:
    logfile:
      class: logging.FileHandler
      filename: magcalc.log
      formatter: simpleFormatter
      delay: True
      level: !!python/name:logging.NOTSET
    console:
      class: logging.StreamHandler
      stream: ext://sys.stdout
      formatter: simpleFormatter
      level: !!python/name:logging.NOTSET

formatters:
  simpleFormatter:
    class: !!python/name:logging.Formatter
    format: '%(levelname)-05s %(name)s@l%(lineno)-4d %(message)s'
    datefmt: '%d/%m/%Y %H:%M:%S'
"""

log = logging.getLogger(__name__)

class MagmomData(object):
    """
    Example:

    >>> data = MagmomData("100010")
    >>> print(data.formated(5))
    """

    def __init__(self, raw_binary_string):

        self.energy = None
        self.raw_binary_string = raw_binary_string # raw binary string such as 10010
        self.jobdir = None

    def formated(self, magmom_value):
        """
        return formated output for MAGMOM value in INCAR file
        """

        vlist = []
        for k in self.raw_binary_string:
            if int(k) == 1:
                v = magmom_value
            elif int(k) == 0:
                v = magmom_value * -1
            else:
                raise RuntimeError("the bit in raw_binary_string has to be 1 or 0 only!")

            vlist.append("{:>4.1f}".format(v))

        s = "  ".join(vlist)
        return s

class MagmomCalculator(object):

    def __init__(self, total_number, magmom_value, incar_template, configfile, jobs_dir=None):
        """
        total_number: the total number of Fe atoms
        magmom_value: the initial value for the magnetic moment as indicated by MAGMOM in vasp INCAR file
        """
        self.total_number = total_number
        self.magmom_value = magmom_value
        self.incar_template = incar_template

        self.jobs_dir = "jobs" if jobs_dir is None else jobs_dir

        self.magmom_db = defaultdict(lambda: None)
        self.old_counts = 0
        key = "MAGCALC_MAX_JOBS"
        self.max_jobs =  int(os.environ[key]) if key in os.environ else 2**(self.total_number - 1)
        self.magmom_seqs = []

        self.run_in_parallel = False
        self.wait_until_complete = False
        self.debug_mode = False
        self.initialized = False

        # clean unnecessary vasp files after job finished
        self.clean_files = False

        ####################### Configuration ########################
        self.configfile = configfile
        self.config_version = 1.2

        self.config = self._get_local_config() or self._get_default_config()

        self.runfile = self.config["Run Script"]["filename"]
        self.runscript = JOB_RUN

        if "random_seed" in self.config:
            random_seed = self.config["random_seed"]

        self.random_seed = random_seed or random.randint(1, 2**(self.total_number - 1))

        # stop searching when target_score reached
        try:
            self.target_score = self.config["target_score"]
        except KeyError:
            self.target_score = -999999999999999999999999.9

        # triadic crossover
        self.current_best = None

    def _get_default_config(self):
        """ always return the default configurations """

        with open(self.configfile, "w") as fp:
            seed = random.randint(1, 2**(self.total_number - 1))
            txt = DEFAULT_CONFIG.format(random_seed=str(seed), config_version=self.config_version)
            fp.write(txt)

        return yaml.load(txt)

    def _get_local_config(self):
        """ load configurations from disk; return None if failed """

        if os.path.exists(self.configfile) and os.path.getsize(self.configfile) > 0:
            config = yaml.load(file(self.configfile))
            if "version" in config and config["version"] >= self.config_version:
                return config
            else:
                log.info("config file version not matched.")
        else:
            log.info("config file {} does not exist or it is empty.".format(self.configfile))

        return None

    def _get_item(self, binarykey):
        """
        get MAGMOM item with binary string as the key

        create and save it into the db if not found
        """

        if self.magmom_db[binarykey] is None:
            self.magmom_seqs.append(binarykey)
            magmom = MagmomData(binarykey)
            self.magmom_db[binarykey] = magmom

        return self.magmom_db[binarykey]

    def show_results(self):
        """ show results in jobs directory """

        import pandas as pd

        if not os.path.exists(self.jobs_dir):
            print("jobs directory {} does not exist!".format(self.jobs_dir))
            return

        lst_dir = []
        lst_energy = []
        lst_seq = []

        for ff in os.listdir(self.jobs_dir):
            if not os.path.isdir(os.path.join(self.jobs_dir, ff)):
                continue

            osz = os.path.join(self.jobs_dir, ff, "OSZICAR")
            energy = self._get_energy_from_oszicar(osz)
            if energy is None:
                continue

            if self.clean_files:
                for f in os.listdir(os.path.join(self.jobs_dir, ff)):
                    if f != "OSZICAR":
                        os.remove(os.path.join(self.jobs_dir, ff, f))
                        print("{} removed.".format(f))

            lst_dir.append(ff)
            lst_energy.append(energy)

        df = pd.DataFrame()
        df["directory"] = lst_dir
        df["energy"] = lst_energy
        # construct the magmom sequences
        mag_bits = "-+"
        df["seqs"] = df.directory.apply(lambda dir: "".join([mag_bits[int(b)] for b in dir]))
        df["net_mag"] = df.directory.apply(lambda x: abs(2*sum([int(i) for i in x]) - 12))
        df.sort_values(by=["energy", "net_mag"], inplace=True)

        print(df)
        df.to_csv("results.csv", index=False)
        print("data saved as results.csv")
        return

    def _get_energy_from_oszicar(self, oszcar_file):
        """ get energy from vasp oszcar file; return None if failed for any reasons"""

        if not os.path.exists(oszcar_file):
            log.debug("{} not found on the disk.".format(oszcar_file))
            return None

        key_line = ""
        def _parse_energy(oszcar_file):
            energy = None
            with open(oszcar_file, "r") as fp:
                lines = fp.readlines()
                line = lines[-1]
                if line.find("E0=") > 0:
                    energy = float(re.search(r'E0=\s*([^\s]+)\s.*', line).groups()[0])

            if self.clean_files and energy is not None:
                with open(oszcar_file, "w") as fp:
                    fp.write(line)

            return energy

        try:
            return _parse_energy(oszcar_file)
        except:
            log.exception("Failed to parse energy from {}".format(oszcar_file))

        return None

    def get_item_energy(self, magmom, must=False):
        """ return vasp calculated energy """

        # use cached value if available
        if magmom.energy is not None:
            return magmom.energy

        # if no cached energy, take it from the disk or calculate it.
        energy = self._get_item_energy_from_disk(magmom)
        # only calculate it when we have to
        if energy is None:
            if must or not self.run_in_parallel:
                energy = self._calc_item_energy(magmom)
            else:
                energy = self._poll_item_energy_from_disk(magmom)

        # cache it
        magmom.energy = energy

        return energy if energy is not None else 0.0

    def _get_item_energy_from_disk(self, magmom):
        jobdir = self._get_item_jobdir(magmom)
        oszcar_file = os.path.join(jobdir, "OSZICAR")

        energy = self._get_energy_from_oszicar(oszcar_file)

        return energy

    def _poll_item_energy_from_disk(self, magmom):
        """ wait until the data to be fed """

        total_minutes = 0.0
        interval = 5           # every 5 seconds

        log.info("wait for {} to be fed".format(magmom.raw_binary_string))
        while self.wait_until_complete:
            energy = self._get_item_energy_from_disk(magmom)
            if energy: return energy
            log.debug("wait for data from {}".format(magmom.raw_binary_string))
            time.sleep(interval)
            total_minutes += interval / 60.0
            if total_minutes >= 90: raise RuntimeError("waiting too long...give up.")

    def _calc_item_energy(self, magmom):
        """ calculate energy by invoking vasp application """

        log.debug("submit job {}".format(magmom.raw_binary_string))
        self._submit_vasp_job(magmom)
        energy = self._get_item_energy_from_disk(magmom)

        # test again
        if energy is None:
            raise RuntimeError("Could not get energy for {}. Did it fail for some reason".format(magmom.raw_binary_string))

        return energy

    def _calc_energy_by_slot_id(self, parallel_slot_id):
        """ test """

        assert type(parallel_slot_id) == type(2)

        self.max_jobs = parallel_slot_id
        seqs = self._create_random_magmom_seqs() if self.config["brute_force"] else self._create_ga_magmom_seqs()

        if 0 <= parallel_slot_id <= len(seqs):
            key = seqs[parallel_slot_id - 1]
            item = self._get_item(key)
            energy = self.get_item_energy(item, must=True)
            if energy:
                s = "slot {slot} finished: {key} = {energy}".format(slot=parallel_slot_id, key=key, energy=energy)
                log.info(s)
            else:
                s = "slot {slot} failed: {key} = {energy}".format(slot=parallel_slot_id, key=key, energy=energy)
                raise RuntimeError(s)

            return True

        raise RuntimeError("wrong parallel slot id is specified: {}/{}".format(parallel_slot_id, len(seqs)))

    def _parallel_by_slot(self):
        VAR = "MAGCALC_SLOT_ID"

        if VAR in os.environ:
            # remove debug information
            log.setLevel(logging.INFO)
            pslot_id = os.environ[VAR]
            log.info("parallel calculation in slot {}".format(pslot_id))
            self._calc_energy_by_slot_id(int(pslot_id))
            return True

        return False

    def _create_random_magmom_seqs(self):
        """ create a full list of magmom sequences in a random way """

        seqs = []
        for v in range(2**(self.total_number - 1)):
            key = "1{value:0{length}b}".format(value=v, length=self.total_number - 1)
            seqs.append(key)

        # make the seqs repeatable by using constant seed
        random.seed(self.random_seed)
        random.shuffle(seqs)

        return seqs

    def _create_ga_magmom_seqs(self):
        """ create a full list of ga sequences for running in parallel """

        seqs = []
        self.run_in_parallel = True

        ga = self._ga_configure()

        try:
            ga.evolve(freq_stats=0)
        except RuntimeError as e:
            log.info("{}".format(e.message))

        log.info("created {} seqs by GA.".format(len(self.magmom_seqs)))
        return self.magmom_seqs

    def _ga_configure(self):
        """ configure GA engine """

        random.seed(self.random_seed)
        log.info("Random seed applied: {}".format(self.random_seed))

        genome = G1DBinaryString.G1DBinaryString(self.total_number - 1)
        genome.evaluator.set(self._ga_evaluate)
        genome.setParams(full_diversity=True)
        genome.setParams(tournamentPool=4)
        genome.initializator.set(self._ga_initializator)

        # connectivity
        try:
            self.nearest_neighbours = self.config["Genetic Algorithm"]["nearest_neighbours"]
            genome.crossover.set(self._ga_crossover6)
        except KeyError:
            genome.crossover.set(self._ga_triadic_crossover)

        genome.mutator.set(self._ga_mutator)

        ga = GSimpleGA.GSimpleGA(genome)

        gens = self.config["Genetic Algorithm"]["generations"]
        ga.setGenerations(gens)

        popsize = self.config["Genetic Algorithm"]["population_size"]
        ga.setPopulationSize(popsize)
        mr = self.config["Genetic Algorithm"]["mutation_rate"]
        ga.setMutationRate(mr)
        cr = self.config["Genetic Algorithm"]["crossover_rate"]
        ga.setCrossoverRate(cr)
        en = self.config["Genetic Algorithm"]["elitism_number"]
        ga.setElitism(True)
        ga.setElitismReplacement(en)
        # ga.selector.set(Selectors.GTournamentSelectorAlternative)
        ga.selector.set(Selectors.GTournamentSelector)
        # ga.internalPop.scaleMethod = Scaling.BoltzmannScaling

        # ga.selector.set(Selectors.GUniformSelector)

        # ga.setMinimax(Consts.minimaxType["minimize"])

        # csv_adapter = DBAdapters.DBFileCSV(identify="run1", filename="stats.csv")
        # ga.setDBAdapter(csv_adapter)

        ga.stepCallback.set(self._ga_evolve_callback)
        ga.convergence_callback.set(self._ga_is_locked)
        ga.prepop_callback.set(self._ga_prepop_callback)
        # ga.terminationCriteria.set(GSimpleGA.ConvergenceCriteria)
        # ga.terminationCriteria.set(self._ga_criteria)

        return ga

    def _ga_prepop_callback(self, ga_engine):
        if self.run_in_parallel:
            log.info("prepoping...")
            self.wait_until_complete = True
            ga_engine.internalPop.evaluate()
        return False

    def _submit_vasp_job(self, magmom):
        """ submit job by using external vasp application """

        self._prepare_vasp_inputs(magmom, self.incar_template)

        jobdir = self._get_item_jobdir(magmom)

        # it is bad to create run script every time
        # especially when run in parallel
        # self.generate_run_script()

        if not os.path.exists(self.runfile):
            raise RuntimeError("{} is not created correctly.".format(self.runfile))

        p = subprocess.Popen([self.runfile],
                             stdout=subprocess.PIPE,
                             shell=True,
                             env=dict(os.environ, XXX_JOBDIR=jobdir))

        out, err = p.communicate()
        if p.returncode != 0:
            log.warn("Error msg: {}".format(err))


    def generate_run_script(self):
        adir = os.path.dirname(self.runfile)
        if not os.path.exists(adir):
            os.makedirs(adir)
        with open(self.runfile, "w") as fp:
            fp.write(self.runscript)

        # make it executable
        os.chmod(self.runfile, 0755)


    def _get_item_jobdir(self, magmom):
        adir = "{}/{}".format(self.jobs_dir, magmom.raw_binary_string)
        return adir

    def _prepare_vasp_inputs(self, magmom, incar_template):
        """
        prepare input files for vasp calculation
        """

        if not os.path.exists(incar_template):
            raise RuntimeError("cannot open {} for read!".format(incar_template))

        # convert to abs path for sure
        incar_template = os.path.abspath(incar_template)

        ##################### replace MAGMOM tag #####################
        lines = []
        magmom_string = magmom.formated(self.magmom_value)
        found = False
        with open(incar_template) as fp:
            for line in fp:
                if line.lower().find("magmom") >= 0:
                    line = re.sub("[xX]+", magmom_string, line)
                    found = True
                lines.append(line)
        if not found:
            raise RuntimeError("Please fill MAGMOM line with XXXX in INCAR for templating.")


        ####################### prepare inputs #######################
        vasp_dir, _ = os.path.split(incar_template)
        poscar_file = os.path.join(vasp_dir, "POSCAR")
        potcar_file = os.path.join(vasp_dir, "POTCAR")
        kpoints_file = os.path.join(vasp_dir, "KPOINTS")

        adir = self._get_item_jobdir(magmom)
        if not os.path.exists(adir):
            os.makedirs(adir)
        # magmom.jobdir = adir

        # INCAR
        new_incar_file = "{}/INCAR".format(adir)
        new_poscar_file = "{}/POSCAR".format(adir)
        new_potcar_file = "{}/POTCAR".format(adir)
        new_kpoints_file = "{}/KPOINTS".format(adir)

        with open(new_incar_file, "w") as fp:
            fp.write("".join(lines))

        # use linux hard link to reduce disk usage
        try:
            os.symlink(poscar_file, new_poscar_file)
            os.symlink(potcar_file, new_potcar_file)
            os.symlink(kpoints_file, new_kpoints_file)
        except:
            pass

    def search(self):
        # self._tmp_test()
        # return
        # for testing only
        if self.debug_mode:
            self._brute_force_vs_ga()
            return

        if self.config["brute_force"]:
            self._search_brute_force()
        else:
            self._search_random_ga()

    def _brute_force_vs_ga(self):
        log.setLevel(logging.INFO)
        seeds = random.sample(range(3000), 100)

        counts_lst = []
        for seed in seeds:
            self.random_seed = seed
            bfs = self._search_brute_force()
            self._reset_cache()
            gas = self._search_random_ga()
            log.info("seed  brute_force  GA ==> {}\t{}\t{}".format(seed, bfs, gas))

    def _tmp_test(self):
        log.setLevel(logging.INFO)
        # txt = open("/home/ybyygu/ToDo/Projects/structure-prediction/磁态优化/plots/data.txt").read()
        txt = open("/home/ybyygu/ToDo/Projects/structure-prediction/磁态优化/tests/mengyu/mag-o2b2/data.txt").read()
        lines = txt.strip().splitlines()
        seeds = []
        for line in lines[1:]:
            seeds.append(int(line.split()[0]))

        for seed in seeds:
            self.random_seed = seed
            bfs = self._search_brute_force()
            # bfs = 1
            self._reset_cache()
            gas = self._search_random_ga()
            log.info("seed  brute_force  GA ==> {:8} {:10} {:10}".format(seed, bfs, gas))

    def _reset_cache(self):

        self.magmom_db = defaultdict(lambda: None)
        self.magmom_seqs = []
        self.old_counts = 0

    def _search_brute_force(self):
        """ generate all possible MAGMOM combinations """

        # for xx in itertools.product(("1", "0"), repeat=self.total_number - 1):
        #     bits = ["1"]
        #     bits.extend(xx)
        #     s = "".join(bits)
        #     # save it for later reference
        #     self._get_item(s)

        if self._parallel_by_slot():
            return

        log.info("searching all possible combinations in a brute force way.")

        keys = self._create_random_magmom_seqs()

        results = []
        for key in keys:
            mgm = self._get_item(key)
            energy = self.get_item_energy(mgm)
            results.append((energy, key))

            s = "{}/{} ==> {}".format(len(self.magmom_db), self.max_jobs, key)
            log.debug(s)
            if len(results) >= self.max_jobs:
                log.info("the maximum allowed combinations have been explored. Stop now.")
                break
            if self.debug_mode and energy <= self.target_score:
                break
            if energy <= self.target_score:
                break

        results.sort(reverse=True)
        for energy, key in results:
            log.info("{} ==> {:.4f}".format(key, energy))

        log.info("Explored combinations: {} of {}".format(len(self.magmom_db), len(keys)))
        log.info("random seed applied: {}".format(self.random_seed))

        return len(results)

    def _search_random_ga(self):
        if self._parallel_by_slot():
            return

        log.info(" GA searching for magnetic states ".center(60, "="))
        ga = self._ga_configure()

        try:
            ga.evolve(freq_stats=0)
        except RuntimeError:
            pass
        best = ga.bestIndividual()

        log.info(" GA searching finished ".center(60, "="))
        log.info("configurations loaded from: {}".format(self.configfile))
        log.info("Explored combinations: {}".format(len(self.magmom_db)))
        log.info("The best individual: 1{} ==> {:.4f}".format(best.getBinary(), best.score*-1))
        log.info("to resume the searching, just set random_seed to {}.".format(self.random_seed))

        energies = []
        for key, item in self.magmom_db.items():
            energy = self.get_item_energy(item, must=True)
            energies.append(energy)
        energies.sort(reverse=True)
        for e in energies:
            print(e)


        return len(self.magmom_db)

    def _ga_mutator(self, genome, **args):
        """ The shuffle or flip mutator for binary strings """

        pmut = args["pmut"]

        if pmut <= 0.0:
            log.debug("mutate nothing.")
            return 0

        if random.random() <= pmut:
            random.shuffle(genome)

        if random.random() <= pmut:
            pos = random.randint(0, len(genome) - 1)
            # invert the bit
            genome[pos] = 1 if genome[pos] == 0 else 0

        return 1

    def _ga_crossover1(self, genome, **args):
        sister = None
        brother = None
        gmom = args["mom"]
        gdad = args["dad"]

        assert gmom != gdad

        size = len(gmom)
        half = int(size / 2.0)
        bits = random.sample(range(len(gmom)), half)

        sister = gmom.clone()
        sister.resetStats()
        for b in bits:
            sister[b] = gdad[b]

        brother = gdad.clone()
        brother.resetStats()
        for b in bits:
            brother[b] = gmom[b]

        return (sister, brother)

    def _ga_crossover2(self, genome, **args):
        sister = None
        brother = None
        gmom = args["mom"]
        gdad = args["dad"]

        assert gmom != gdad

        size = len(gmom)
        # record the position that pointing to different value between gmom and gdad
        pos_diffs = []
        for pos in range(size):
            if gmom[pos] != gdad[pos]:
                pos_diffs.append(pos)

        sister = gmom.clone()
        sister.resetStats()
        brother = gdad.clone()
        brother.resetStats()

        # randomly swaping the bits
        for pos in pos_diffs:
            if random.random() > 0.5:
                sister[pos] = gdad[pos]
                brother[pos] = gmom[pos]

        # log.debug("bits: {}".format("".join((str(b) for b in bits))))
        # log.debug("mom {} ==> {}".format(gmom.getBinary(), gmom.score))
        # log.debug("dad {} ==> {}".format(gdad.getBinary(), gdad.score))
        # log.debug("bro {} ==> {}".format(brother.getBinary(), brother.score))
        # log.debug("sis {} ==> {}".format(sister.getBinary(), sister.score))

        return (sister, brother)

    def _ga_triadic_crossover(self, genome, **args):
        log.debug("crossover using triadic crossover")
        sister = None
        brother = None
        gmom = args["mom"]
        gdad = args["dad"]
        size = len(gmom)

        assert gmom != gdad

        sister = gmom.clone()
        sister.resetStats()
        brother = gdad.clone()
        brother.resetStats()

        # create mask from current_best after mutation
        nbit_mutation = int(size / 10.)
        if nbit_mutation <= 0:
            nbit_mutation = 1
        positions_mutation = random.sample(list(range(size)), nbit_mutation)
        mask = []
        for i in range(size):
            v = self.current_best[i]
            if i in positions_mutation:
                if v == 1:
                    v = 0
                else:
                    v = 1
            mask.append(v)

        # record the positions in different spin direction between mask and dad
        positions_different_spin = []

        for i in range(size):
            if gdad[i] != mask[i]:
                positions_different_spin.append(i)

        # swap the spin according to the mask
        for i in range(size):
            if i in positions_different_spin:
                sister[i] = gdad[i]
                brother[i] = gmom[i]

        return (sister, brother)


    def _ga_crossover3(self, genome, **args):
        # log.debug("crossover using _ga_crossover3")
        sister = None
        brother = None
        gmom = args["mom"]
        gdad = args["dad"]

        assert gmom != gdad

        size = len(gmom)
        mset = set([])
        dset = set([])
        for i in range(size):
            if gdad[i] == 1:
                dset.add(i)
            if gmom[i] == 1:
                mset.add(i)
        common = mset.intersection(dset)
        diffs = (mset - dset).union(dset - mset)
        lst = random.sample(diffs, len(mset) - len(common))
        bits_sister = common.union(lst)
        lst = random.sample(diffs, len(dset) - len(common))
        bits_brother = common.union(lst)

        sister = gmom.clone()
        sister.resetStats()
        brother = gdad.clone()
        brother.resetStats()

        for i in range(size):
            sister[i] = 1 if i in bits_sister else 0
            brother[i] = 1 if i in bits_brother else 0

        # log.debug("bits: {}".format("".join((str(b) for b in bits))))
        # log.debug("mom {} ==> {}".format(gmom.getBinary(), gmom.score))
        # log.debug("dad {} ==> {}".format(gdad.getBinary(), gdad.score))
        # log.debug("bro {} ==> {}".format(brother.getBinary(), brother.score))
        # log.debug("sis {} ==> {}".format(sister.getBinary(), sister.score))

        return (sister, brother)

    def _ga_crossover4(self, genome, **args):
        """ nearest neighbor crossover """

        log.debug("crossover using _ga_crossover4")

        NN = {
            1:12,
            2:3,
            3:2,
            4:5,
            5:4,
            6:7,
            7:6,
            8:9,
            9:8,
            10:11,
            11:10,
            12:1
            }
        sister = None
        brother = None
        gmom = args["mom"]
        gdad = args["dad"]

        assert gmom != gdad

        size = len(gmom)
        mset = set([])
        dset = set([])
        for i in range(size):
            if gdad[i] == 1:
                dset.add(i)
            if gmom[i] == 1:
                mset.add(i)

        size = len(gmom)
        pos_mom = set([])
        pos_dad = set([])
        for i in range(size):
            if gdad[i] == 1:
                pos_dad.add(i)
            if gmom[i] == 1:
                pos_mom.add(i)

        pos_all = pos_mom.union(pos_dad)
        pos_common = pos_mom.intersection(pos_dad)
        log.debug("mom: {}".format(sorted(pos_mom)))
        log.debug("dad: {}".format(sorted(pos_dad)))
        log.debug("all: {}".format(sorted(pos_all)))
        log.debug("common: {}".format(sorted(pos_common)))

        def _pos2bits(pos):
            """ change position list into binary bits """
            size = len(gmom)
            lst = [0 for i in range(size)]
            for v in pos:
                lst[v] = 1

            return lst

        def _get_bit(lst, pos):
            return 1 if pos < 0 else lst[pos]

        def _check_bits(bits):
            assert len(gmom) == len(gdad) == len(bits)
            score = 0
            for a in NN:
                v = NN[a]
                id1, id2 = a - 2, v - 2
                mbit1, mbit2 = _get_bit(gmom, id1), _get_bit(gmom, id2)
                dbit1, dbit2 = _get_bit(gdad, id1), _get_bit(gdad, id2)
                bit1, bit2 = _get_bit(bits, id1), _get_bit(bits, id2)
                if mbit1 == mbit2 and dbit1 == dbit2:
                    if bit1 == bit2:
                        score += 0
                elif mbit1 != mbit2 and dbit1 != dbit2:
                    if bit1 != bit2:
                        score += 0
                        if mbit1 == dbit1 and mbit1 == bit1:
                            score += 1
                else:
                    pass
            return score

        def _get_candidate(pos_all, num):
            all = []
            for vv in itertools.combinations(pos_all, num):
                lst = _pos2bits(vv)
                score = _check_bits(lst)
                all.append((score, vv))
            all.sort(reverse=True)
            if len(all) > 3:
                p = random.sample(all[:3], 1)
                score, vv = p[0]
            else:
                score, vv = all[0]
            return vv, score

        bits_brother, score1 = _get_candidate(pos_all, len(pos_dad))
        bits_sister, score2 = _get_candidate(pos_all, len(pos_mom))
        log.debug("brother: {}/{}".format(sorted(bits_brother), score1))
        log.debug("sister: {}/{}".format(sorted(bits_sister), score2))

        sister = gmom.clone()
        sister.resetStats()
        brother = gdad.clone()
        brother.resetStats()

        for i in range(size):
            sister[i] = 1 if i in bits_sister else 0
            brother[i] = 1 if i in bits_brother else 0

        # log.info("mom {}/{} ==> {}".format(gmom.getBinary(), int("1"+gmom.getBinary(), 2), gmom.score))
        # log.info("dad {}/{} ==> {}".format(gdad.getBinary(), int("1"+gdad.getBinary(), 2), gdad.score))
        # log.info("bro {}/{} ==> {}".format(brother.getBinary(), int("1"+brother.getBinary(), 2), brother.score))
        # log.info("sis {}/{} ==> {}".format(sister.getBinary(), int("1"+sister.getBinary(), 2), sister.score))

        return (sister, brother)

    def _ga_crossover5(self, genome, **args):
        """ nearest neighbor crossover """

        log.debug("crossover using _ga_crossover5")

        NN = {
            1:12,
            2:3,
            3:2,
            4:5,
            5:4,
            6:7,
            7:6,
            8:9,
            9:8,
            10:11,
            11:10,
            12:1
            }

        sister = None
        brother = None
        gmom = args["mom"]
        gdad = args["dad"]

        assert gmom != gdad

        size = len(gmom)
        mset = set([])
        dset = set([])
        for i in range(size):
            if gdad[i] == 1:
                dset.add(i)
            if gmom[i] == 1:
                mset.add(i)

        size = len(gmom)
        pos_mom = set([])
        pos_dad = set([])
        for i in range(size):
            if gdad[i] == 1:
                pos_dad.add(i)
            if gmom[i] == 1:
                pos_mom.add(i)

        pos_all = pos_mom.union(pos_dad)
        pos_common = pos_mom.intersection(pos_dad)
        log.debug("mom: {}".format(sorted(pos_mom)))
        log.debug("dad: {}".format(sorted(pos_dad)))
        log.debug("all: {}".format(sorted(pos_all)))
        log.debug("common: {}".format(sorted(pos_common)))

        lst_bonds = []
        mom = [x for x in gmom]
        mom.insert(0, 1)
        dad = [x for x in gdad]
        dad.insert(0, 1)
        for x1, x2 in NN.items():
            # p1: current id, p2: connected id
            p1, p2 = x1 -1, x2 - 1
            corr = "F"
            if mom[p1] == mom[p2] and dad[p1] == dad[p2] and mom[p1] == dad[p1]:
                corr = "T"
            elif mom[p1] != mom[p2] and dad[p1] != dad[p2] and mom[p1] == dad[p1]:
                corr = "T"

            if corr == "F":
                print("{}:{} --> {}".format(x1, x2, corr))
                if p1 < p2:
                    lst_bonds.append((p1, p2))

        print(lst_bonds)

        sister = copy.copy(mom)
        brother = copy.copy(dad)
        b1, b2 = random.choice(lst_bonds)
        print(b1, b2)

        bb1, bb2 = brother[b1], brother[b2]
        if random.random() > 0.5:
            x1, x2 = b1, b2
        else:
            x1, x2 = b2, b1
            brother[b1], brother[b2] = sister[x1], sister[x2]
            sister[b1], sister[b2] = bb1, bb2

        gsister = gmom.clone()
        gsister.resetStats()
        gbrother = gdad.clone()
        gbrother.resetStats()

        for i, (v1, v2) in enumerate(zip(brother[1:], sister[1:])):
            gbrother[i] = v1
            gsister[i] = v2

        print(brother)
        print(sister)
        log.debug("mom {}/{} ==> {}".format(gmom.getBinary(), int("1"+gmom.getBinary(), 2), gmom.score))
        log.debug("dad {}/{} ==> {}".format(gdad.getBinary(), int("1"+gdad.getBinary(), 2), gdad.score))
        log.debug("bro {}/{} ==> {}".format(gbrother.getBinary(), int("1"+gbrother.getBinary(), 2), gbrother.score))
        log.debug("sis {}/{} ==> {}".format(gsister.getBinary(), int("1"+gsister.getBinary(), 2), gsister.score))

        return (gsister, gbrother)

    def _ga_crossover6(self, genome, **args):
        """ nearest neighbors crossover """

        log.debug("crossover using _ga_crossover4")

        NN = self.nearest_neighbours
        
        sister = None
        brother = None
        gmom = args["mom"]
        gdad = args["dad"]

        assert gmom != gdad

        size = len(gmom)
        mset = set([])
        dset = set([])
        for i in range(size):
            if gdad[i] == 1:
                dset.add(i)
            if gmom[i] == 1:
                mset.add(i)

        size = len(gmom)
        pos_mom = set([])
        pos_dad = set([])
        for i in range(size):
            if gdad[i] == 1:
                pos_dad.add(i)
            if gmom[i] == 1:
                pos_mom.add(i)

        pos_all = pos_mom.union(pos_dad)
        pos_common = pos_mom.intersection(pos_dad)
        log.debug("mom: {}".format(sorted(pos_mom)))
        log.debug("dad: {}".format(sorted(pos_dad)))
        log.debug("all: {}".format(sorted(pos_all)))
        log.debug("common: {}".format(sorted(pos_common)))

        def _pos2bits(pos):
            """ change position list into binary bits """
            size = len(gmom)
            lst = [0 for i in range(size)]
            for v in pos:
                lst[v] = 1

            return lst

        def _get_bit(lst, pos):
            return 1 if pos < 0 else lst[pos]

        def _check_bits(bits):
            assert len(gmom) == len(gdad) == len(bits)
            score = 0
            for a in NN:
                for v in NN[a]:
                    id1, id2 = a - 2, v - 2
                    if id1 > id2: continue
                    
                    mbit1, mbit2 = _get_bit(gmom, id1), _get_bit(gmom, id2)
                    dbit1, dbit2 = _get_bit(gdad, id1), _get_bit(gdad, id2)
                    bit1, bit2 = _get_bit(bits, id1), _get_bit(bits, id2)
                    if mbit1 == mbit2 and dbit1 == dbit2 and bit1 == bit2:
                        if mbit1 == bit1:
                            score += 1
                        else:
                            score += 0.5
            return score

        def _get_candidate(pos_all, num):
            all = []
            for vv in itertools.combinations(pos_all, num):
                lst = _pos2bits(vv)
                score = _check_bits(lst)
                all.append((score, vv))
            all.sort(reverse=True)
            if len(all) > 5:
                p = random.sample(all[:5], 1)
                score, vv = p[0]
            else:
                score, vv = all[0]
            return vv, score

        bits_brother, score1 = _get_candidate(pos_all, len(pos_dad))
        bits_sister, score2 = _get_candidate(pos_all, len(pos_mom))
        log.debug("brother: {}/{}".format(sorted(bits_brother), score1))
        log.debug("sister: {}/{}".format(sorted(bits_sister), score2))

        sister = gmom.clone()
        sister.resetStats()
        brother = gdad.clone()
        brother.resetStats()

        for i in range(size):
            sister[i] = 1 if i in bits_sister else 0
            brother[i] = 1 if i in bits_brother else 0

        # log.info("mom {}/{} ==> {}".format(gmom.getBinary(), int("1"+gmom.getBinary(), 2), gmom.score))
        # log.info("dad {}/{} ==> {}".format(gdad.getBinary(), int("1"+gdad.getBinary(), 2), gdad.score))
        # log.info("bro {}/{} ==> {}".format(brother.getBinary(), int("1"+brother.getBinary(), 2), brother.score))
        # log.info("sis {}/{} ==> {}".format(sister.getBinary(), int("1"+sister.getBinary(), 2), sister.score))

        return (sister, brother)

    def _ga_initializator(self, genome, **args):
        """ 1D Binary String initializator """

        size = genome.getListSize()
        seed = args["seed"]
        log.debug("Initializing using seed {}".format(seed))
        # seed = random.randint(0, 2**(size))

        # use defined initial magmom seqs
        try:
            initial_seqs = self.config["Genetic Algorithm"]["initial_magmom_seqs"]
            bits = []
            
            ms = "+" if initial_seqs[0] == "+" else "-"
            for x in initial_seqs[1:]:
                if x == ms:
                    bits.append(1)
                else:
                    bits.append(0)
            assert len(bits) == size

            if self.initialized:
                random.shuffle(bits)
            else:
                self.initialized = True
        except KeyError:
            binary_string = "{value:0{length}b}".format(value=seed, length=size)
            bits = [int(b) for b in binary_string]
            
        genome.setInternalList(bits)
        return

    def _ga_evaluate(self, chromosome):
        """
        evaluate function for GA search
        """
        key = "1" + chromosome.getBinary()

        mgm = self._get_item(key)

        # check if all possible combinations have been explored
        if len(self.magmom_db) < self.max_jobs:
            s = "{}/{} ==> {}".format(len(self.magmom_db), self.max_jobs, key)
            log.debug(s)
        else:
            # print(self.magmom_seqs)
            raise RuntimeError("the maximum allowed combinations have been explored. Stop now.")

        score = -1 * self.get_item_energy(mgm)

        return score

    # def _ga_criteria(self, ga_engine):
    #     pop = ga_engine.getPopulation()
    #     indv = pop.bestRaw()
    #     score = indv.score
    #     cond1 = score > 205.0

    #     return cond1

    def _ga_evolve_callback(self, ga_engine):
        """
        The step callback function, this function
        will be called every step (generation) of the GA evolution
        """

        generation = ga_engine.getCurrentGeneration()
        log.debug("==== Current generation: {}".format(generation))
        pop = ga_engine.getPopulation()

        for p in pop:
            key = p.getBinary()
            s = "1{key}/{decimal:<{pl}d} ==> {score:10.5f}".format(key=key, score=p.score, decimal=int(key, 2), pl=len(str(2**self.total_number)))
            log.debug(s)

        # log.info(ga_engine.getStatistics())

        indv = pop.bestRaw()
        s = "current best: 1{} ==> {}".format(indv.getBinary(), indv.score)
        log.debug(s)
        s = "Calculated jobs: {}".format(len(self.magmom_db))
        log.debug(s)

        pop = ga_engine.getPopulation()
        indv = pop.bestRaw()
        score = indv.score
        # store for triadic crossover
        self.current_best = indv

        if self.debug_mode:
            return score > self.target_score

        return score > self.target_score*-1
        return False

    def _ga_is_locked(self, *args):

        if len(self.magmom_db) - self.old_counts < 2:
            self.old_counts = len(self.magmom_db)
            return True

        self.old_counts = len(self.magmom_db)
        return False


# FIXME: maybe useful someday
def shift(binarykey):
    last_bit = binarykey[-1]
    xx = int(binarykey, 2) >> 1
    ss = last_bit + str(bin(xx))[2:]
    print ss

def random_search(incar_template, fe_number, magmom_value=5.0, configfile="magcalc.conf", debug=False):
    if os.path.exists(incar_template):
        print("using {:} as a template...".format(incar_template))
    else:
        print("{} does not exist!".format(incar_template))
        return 1

    magcalc = MagmomCalculator(
        incar_template=incar_template,
        total_number=fe_number,
        magmom_value=magmom_value,
        configfile=configfile
    )

    if debug:
        magcalc.debug_mode = True

    restore_ld_library_path()

    magcalc.search()

def show_finished_results(directory, clean=False):
    print("Showing finished results in {}/:".format(directory))

    magcalc = MagmomCalculator(
        incar_template="template/INCAR",
        total_number=5,
        magmom_value=4.0,
        configfile="magcalc.conf",
        jobs_dir=directory
    )

    magcalc.clean_files = clean
    magcalc.show_results()


def create_files(incar_template, fe_number, magmom_value=5.0, configfile="magcalc.conf"):
    print("using {:} as a template...".format(incar_template))

    magcalc = MagmomCalculator(
        incar_template=incar_template,
        total_number=fe_number,
        magmom_value=magmom_value,
        configfile=configfile)

    prog = os.path.basename(__file__)

    # remove file extension
    prog, _ = os.path.splitext(prog)

    cmdline = "{} -r -t {} {} {}".format(prog, incar_template, fe_number, magmom_value)
    txt = PBS_FILE.format(magcalc_cmdline=cmdline)
    with open("run.pbs", "w") as fp:
        fp.write(txt)

    magcalc.generate_run_script()

    print("script files created. please change {} to fit your system setup.".format(configfile))
    print("when ready, submit it with qsub run.pbs")

def restore_ld_library_path(alt_ld_path_var_name="OLD_LIBRARY_PATH"):
    if alt_ld_path_var_name in os.environ:
        os.environ["LD_LIBRARY_PATH"] = os.environ[alt_ld_path_var_name]
        # log.debug("restored LD_LIBRARY_PATH from {}.".format(alt_ld_path_var_name))
        return

    log.debug("{} not found in os.environ.".format(alt_ld_path_var_name))

def main(argv=None):
    """ main function """

    if argv is None: argv = sys.argv

    # parse commandline options

    version = "%(prog)s " + __VERSION__ + "; updated at: " + __UPDATED__
    desc = "magnetization searching for Fe-containing systems"
    cmdl_parser = argparse.ArgumentParser(description=desc)
    cmdl_parser.add_argument('-v', '--version',
                             version=version,
                             action='version')

    cmdl_parser.add_argument('-t', '--template',
                             action='store',
                             nargs=3,
                             metavar=('INCAR-file', 'number-of-Fe', 'initial-magmom'),
                             help='generate INCAR files by changing MAGMOM line')

    cmdl_parser.add_argument('-r', '--run',
                             action='store_true',
                             help='run vasp calculation')

    cmdl_parser.add_argument('-d', '--debug',
                             action='store_true',
                             help='for debug and test only')

    cmdl_parser.add_argument('-c', '--clean',
                             action='store_true',
                             default=False,
                             help='clean unnecessary vasp files if job finished; to use in combination with the "-p" option')

    # parse finished results in specific directory, if omitted jobs
    # will be the default
    cmdl_parser.add_argument('-p', '--print',
                             action='store',
                             dest='printr',
                             nargs='?',
                             const='jobs',
                             metavar='jobs directory',
                             default=False,
                             help='print finished results in optional jobs directory')

    cmdl_args = cmdl_parser.parse_args()

    if len(sys.argv) == 1:
        cmdl_parser.print_help()
        return

    if cmdl_args.printr:
        show_finished_results(cmdl_args.printr, cmdl_args.clean)
        return

    if cmdl_args.template:
        incar_file, fe_number, magvalue = cmdl_args.template
        if cmdl_args.run:
            random_search(incar_file, int(fe_number), float(magvalue), debug=cmdl_args.debug)
        else:
            create_files(incar_file, int(fe_number), float(magvalue))
    else:
        cmdl_parser.print_help()

if (__name__ == "__main__"):
    config = yaml.load(StringIO.StringIO(YAML_CONF))
    logging.config.dictConfig(config)

    log = logging.getLogger("magcalc")

    result = main()


# Emacs:
# Local Variables:
# time-stamp-pattern: "100/^__UPDATED__ = '%%'$"
# End:
